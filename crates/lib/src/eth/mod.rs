/* alloy-backed ethereum client behind loop's own types: chain primitives
carry schemas, calls have blocking + _async twins, and the connection is a
startup-initialized global, all mirroring the database module */

pub mod abi;
mod client;
mod config;
mod contract;
mod error;
mod events;
mod global;
mod signer;
mod tx;
mod types;
#[cfg(test)]
mod tests;

pub use client::Eth;
pub use config::Config;
pub use contract::{CallBuilder, TxBuilder};
pub use error::EthError;
pub use events::{BlockStream, ContractEvent, EventStream};
pub use global::{
    balance, balance_async, block_number, block_number_async, blocks, gas_price, gas_price_async,
    get, init, init_from_env, transaction_count, transaction_count_async, treasury, try_get,
    try_treasury,
};
pub use signer::{Signer, Treasury, generate_key};
pub use tx::{TxHandle, TxRequest};
pub use types::{Address, Block, IntoAddress, Receipt, U256, Wei};

/// Entry point for #[contract]-generated event streams; hidden because the
/// generated code is its only intended caller.
#[doc(hidden)]
pub fn __watch<E: ContractEvent>(address: Address) -> Result<EventStream<E>, EthError> {
    events::watch::<E>(address)
}
