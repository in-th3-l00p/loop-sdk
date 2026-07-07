# erc20-dashboard

The ethereum SDK in five endpoints: account balances and gas prices with
schema-validated `Address`/`Wei` primitives, a typed USDC binding generated
from `abi/erc20.json` by `#[contract]`, live `Transfer` events over SSE, and
chain heads over WebSocket.

## run

Point `ETH_RPC_URL` at any mainnet rpc and start the dev server:

```sh
ETH_RPC_URL=https://eth.llamarpc.com loop dev
```

## try it

```sh
# ether balance (checksummed or lowercase; bad addresses are a 400)
curl localhost:3000/balance/0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045

# current gas price
curl localhost:3000/gas

# usdc balance through the typed contract binding
curl localhost:3000/usdc/0x37305B1cD40574E4C5Ce33f8e8306Be057fD7341

# usdc transfers as they land on-chain (sse; usually a few per block)
curl -N localhost:3000/transfers/0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48

# new chain heads (~12s apart)
websocat ws://localhost:3000/heads
```

Sending transactions needs a funded server wallet: run `loop eth wallet new`,
fund the printed address, export the key, and uncomment `[eth.treasury]` in
`loop.toml`. A write then looks like:

```rust
#[rest(post, "/tip")]
fn tip(to: Address, amount: U256) -> Result<TxHandle, HandlerError> {
    Ok(Erc20::at(USDC).transfer(to, amount).from(eth::treasury()).send()?)
}
```
