## A Wolf In Sheep's Coding

The Wolf Game launched with vulnerable GameFi mechanics that could have been exploited through weak PRNG manipulation. Using a simple contract interface `Sheepdog.sol` and a script to submit handled bundles to Flashbots, one could game the `random()` function by requesting the tx be mined only under certain conditions where a valuable trait was rolled (else free reverts), thus continuously submitting until the desired conditions were met. 

Due to the nature of executing randomness on-chain, certain rarer (more valuable) traits took much longer to 'land' on, by some non-linear increment. Thus, one would have to consider this as a trade-off, which scaled up significantly when trying to bundle multiple rare traits together in a single mint. Rolling an 8 "Alpha" for example would have taken multiple days, if not weeks, to execute. 

Error handling and tx simulation was also needed to ensure a Wolf was rolled instead of a sheep, and further that a Wolf was not going to be 'stolen' after minting (another random hazard in the game).

## Configuration

This tool is configured via the following environment variables:

```
ETH_RPC_WS=ws://your-eth-client-websockets:port
PRIVATE_KEY=private key of wallet to register with
FLASHBOTS_KEY=signing key to use for flashbots bundles
SIMULATE_ONLY=boolean, only simulate bundles if true
REGISTRATIONS_FILE=.ron file to read registration info from
RUST_LOG(optional)=Log level
```

