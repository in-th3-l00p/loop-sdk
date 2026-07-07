/* demonstrates the ethereum SDK: chain primitives in endpoint signatures,
typed contract bindings derived from an abi, and on-chain activity streamed
over sse/websocket — all against any public mainnet rpc */

use lib::eth;
use lib::prelude::*;

// usdc on mainnet; the abi file drives the generated methods below
#[contract("abi/erc20.json")]
struct Erc20;

const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

/// Ether balance of any account. A malformed address is a 400 before this
/// body runs — `Address` validates at the wire boundary.
#[rest(get, "/balance/{address}")]
fn balance(address: Address) -> Result<Wei, HandlerError> {
    Ok(eth::balance(address)?)
}

/// Current gas price in wei.
#[rest(get, "/gas")]
fn gas() -> Result<Wei, HandlerError> {
    Ok(eth::gas_price()?)
}

/// USDC balance of any holder, read through the typed contract binding.
#[rest(get, "/usdc/{holder}")]
fn usdc_balance(holder: Address) -> Result<U256, HandlerError> {
    Ok(Erc20::at(USDC).balance_of(holder).call()?)
}

/// Live transfer feed for any erc-20 token, streamed as they land on-chain.
/// `Transfer` is generated from the abi's event definition.
#[sse("/transfers/{token}")]
fn transfers(token: Address) -> Result<impl Iterator<Item = Transfer>, HandlerError> {
    Ok(Erc20::at(token).events::<Transfer>()?)
}

/// New chain heads over websocket.
#[live("/heads")]
fn heads() -> Result<impl Iterator<Item = eth::Block>, HandlerError> {
    Ok(eth::blocks()?)
}

fn main() {
    lib::server::run();
}
