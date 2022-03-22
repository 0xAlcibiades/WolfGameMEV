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