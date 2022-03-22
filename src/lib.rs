use std::env;
use std::ops::Not;
use std::str::FromStr;
use std::string::String;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::{prelude::*, types::Address, utils::keccak256};
use ethers_flashbots::*;
use ethers_providers::Ws;
use log::Level::Debug;
use log::{debug, info, log_enabled};
use serde::Deserialize;
use url::Url;
use ethers_core::types::transaction::eip1559::Eip1559TransactionRequest;

pub(crate) const WOOLF: &str = "0xEB834ae72B30866af20a6ce5440Fa598BfAd3a42";
// TODO(Correct address)
pub(crate) const SHEEPDOG: &str = "0x1bEc112D5AF1f20eD0557A2EDbd5C72e202A9680";

// Generate the type-safe contract bindings by providing the ABI
// definition in json.
abigen!(
    Sheepdog,
    "./src/abi/sheepdog.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

abigen!(
    Woolf,
    "./src/abi/woolf.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

/// Whether we want to simulate + send or just simulate transactions on the relay
#[derive(Debug, Copy, Clone)]
pub enum OperationMode {
    Send,
    Simulate,
}

/// Runtime configuration details for the bot.
pub struct Config {
    pub executor_pk: String,
    pub flashbots_pk: String,
    pub ws_rpc: String,
    pub operation_mode: OperationMode,
}

/// Implementation for config
/// The config is read from shell environment variables.
impl Config {
    pub fn new() -> Result<Config> {
        let ws_rpc = env::var("ETH_RPC_WS").context("Set the ETH_RPC_WS environment variable.")?;
        let executor_pk = env::var("PRIVATE_KEY")
            .context("Set the PRIVATE_KEY environment variable.")?[2..]
            .to_string();
        let flashbots_pk = env::var("FLASHBOTS_KEY")
            .context("Set the FLASHBOTS_KEY environment variable.")?[2..]
            .to_string();
        let operation_mode = env::var("SIMULATE_ONLY");
        let operation_mode = match operation_mode {
            Err(_) => OperationMode::Send,
            Ok(_) => {
                info!("Running in simulation only mode.");
                OperationMode::Simulate
            }
        };
        Ok(Config {
            executor_pk,
            flashbots_pk,
            ws_rpc,
            operation_mode,
        })
    }
}

struct AlphaRoller {
    operation_mode: OperationMode,
    provider: Arc<Provider<Ws>>,
    wallet: LocalWallet,
    flashbots_wallet: LocalWallet,
    client: Arc<SignerMiddleware<FlashbotsMiddleware<Arc<Provider<Ws>>, LocalWallet>, LocalWallet>>,
    woolf: Woolf<
        ethers::prelude::SignerMiddleware<
            FlashbotsMiddleware<Arc<Provider<Ws>>, LocalWallet>,
            LocalWallet,
        >,
    >,
    sheepdog: Sheepdog<
        ethers::prelude::SignerMiddleware<
            FlashbotsMiddleware<Arc<Provider<Ws>>, LocalWallet>,
            LocalWallet,
        >,
    >,
}

/// Implementation of an alpha roller bot.
impl AlphaRoller {
    /// Returns a new bot.
    pub async fn new(config: &Config) -> Result<AlphaRoller> {
        // Setup a websocket connection to geth client.
        let operation_mode = config.operation_mode;
        let ws = Ws::connect(&config.ws_rpc).await?;
        let provider = Provider::new(ws).interval(Duration::from_millis(100));
        let provider = Arc::new(provider);

        // Setup wallet
        let wallet: LocalWallet =
            LocalWallet::from_str(&*config.executor_pk).context("Invalid private key")?;

        // Flashbots signer wallet
        let flashbots_wallet: LocalWallet =
            LocalWallet::from_str(&config.flashbots_pk).context("Invalid flashbots key")?;

        // Setup the flashbots middleware
        let flashbots_middleware: FlashbotsMiddleware<Arc<Provider<Ws>>, LocalWallet> =
            FlashbotsMiddleware::new(
                provider.clone(),
                Url::parse("https://relay.flashbots.net")?,
                flashbots_wallet.clone(),
            );

        // Setup Ethereum client with flashbots middleware
        let client: SignerMiddleware<
            FlashbotsMiddleware<Arc<Provider<Ws>>, LocalWallet>,
            LocalWallet,
        > = SignerMiddleware::new(flashbots_middleware, wallet.clone());
        let client: Arc<
            SignerMiddleware<FlashbotsMiddleware<Arc<Provider<Ws>>, LocalWallet>, LocalWallet>,
        > = Arc::new(client);

        // Setup woolf
        let woolf_address: Address = WOOLF.parse().unwrap();
        let woolf = Woolf::new(woolf_address, client.clone());

        // Setup woolf
        let sheepdog_address: Address = SHEEPDOG.parse().unwrap();
        let sheepdog = Sheepdog::new(sheepdog_address, client.clone());

        Ok(AlphaRoller {
            operation_mode,
            provider,
            wallet,
            flashbots_wallet,
            client,
            woolf,
            sheepdog,
        })
    }

    /// Return a new flashbots bundle request for this block
    async fn new_bundle_request(&self) -> Result<BundleRequest> {
        let block = self.client.get_block_number().await?;
        let mut bundle = BundleRequest::new();
        bundle = bundle.set_simulation_block(block);
        bundle = bundle.set_block(block + 1);
        let now = SystemTime::now();
        bundle = bundle.set_simulation_timestamp(now.duration_since(UNIX_EPOCH)?.as_secs());
        Ok(bundle)
    }

    async fn process_bundle_request(&self, bundle: BundleRequest) -> Result<()> {
        dbg!(bundle.transactions());
        if bundle.transactions().is_empty().not() {
            match self.operation_mode {
                OperationMode::Send => {
                    let pending_bundle = self.client.inner().send_bundle(&bundle).await?;
                    //match pending_bundle.await {
                    //    Ok(bundle_hash) => println!(
                    //        "Bundle with hash {:?} was included in target block",
                    //        bundle_hash
                    //    ),
                    //    Err(PendingBundleError::BundleNotIncluded) => {
                    //        println!("Bundle was not included in target block.")
                   //     }
                   //     Err(e) => println!("An error occured: {}", e),
                   // }
                }
                OperationMode::Simulate => {
                    let simulated_bundle = self.client.inner().simulate_bundle(&bundle);
                    match simulated_bundle.await {
                        Ok(res) => debug!("Simulated bundle: {:?}", res),
                        Err(e) => debug!("Bundle simulation failed: {}", e),
                    }
                }
            }
        }
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        // Get stream of blocks.
        let mut block_stream = self.provider.watch_blocks().await?;

        while block_stream.next().await.is_some() {
            // For each block:
            info!(
                "Got block: {}",
                self.provider
                    .get_block(BlockNumber::Latest)
                    .await
                    .unwrap()
                    .unwrap()
                    .number
                    .unwrap()
            );

            // Prepare an empty bundle request
            let mut bundle_request = self.new_bundle_request().await?;

            let mut nonce = self
                .client
                .get_transaction_count(
                    self.wallet.address(),
                    Some(BlockId::from(BlockNumber::Latest)),
                )
                .await?;

            // Add check tx to bundle
            let check_tx = {
                let mut call = self.sheepdog.method::<_, H256>("roll_alpha", ())?;

                call.tx.set_nonce(nonce);
                call.tx.set_gas_price(U256::from(300000000000u64));
                call.tx.set_gas(U256::from(50000));
                let mut inner: TypedTransaction = call.tx;
                inner
            };
            let check_signature = self.client.signer().sign_transaction(&check_tx).await?;
            bundle_request = bundle_request.push_transaction(
                check_tx.rlp_signed(self.client.signer().chain_id(), &check_signature),
            );

            // Add mint tx to bundle
            let mint_tx = {
                let mut call = self
                    .woolf
                    .method::<_, U256>("mint", (U256::from(1), false))?;
                call.tx.set_nonce(nonce + U256::from(1));
                call.tx.set_gas_price(U256::from(300000000000u64));
                call.tx.set_gas(U256::from(400000));
                let mut inner: TypedTransaction = call.tx;
                inner
            };
            let mint_signature = self.client.signer().sign_transaction(&mint_tx).await?;
            bundle_request = bundle_request.push_transaction(
                mint_tx.rlp_signed(self.client.signer().chain_id(), &mint_signature),
            );

            info!("Processing alpha minting bundle request.");

            self.process_bundle_request(bundle_request).await?;
        }

        Ok(())
    }
}

// This is wrapped up in a thread pool for call by the binary.
#[tokio::main]
pub async fn run(config: &Config) -> Result<()> {
    let mut alpha_roller = AlphaRoller::new(config).await?;

    // Run the roller
    alpha_roller.run().await?;

    // Exit cleanly
    Ok(())
}
