use std::sync::OnceLock;

use super::client::Eth;
use super::config::Config;
use super::error::EthError;
use super::signer::Treasury;
use super::types::{Address, Wei};

static ETH: OnceLock<Eth> = OnceLock::new();

pub async fn init(config: &Config) -> Result<&'static Eth, EthError> {
    let eth = Eth::connect(config).await?;
    ETH.set(eth)
        .map_err(|_| EthError::Unavailable("eth already initialized".into()))?;
    Ok(ETH.get().expect("just set"))
}

pub async fn init_from_env() -> Result<Option<&'static Eth>, EthError> {
    match Config::from_env() {
        Some(config) => init(&config).await.map(Some),
        None => Ok(None),
    }
}

pub fn try_get() -> Option<&'static Eth> {
    ETH.get()
}

pub fn get() -> &'static Eth {
    try_get().expect(
        "eth not initialized: set LOOP_ETH_RPC_URL or add [eth] to loop.toml \
         so the server connects at startup",
    )
}

pub fn try_treasury() -> Option<&'static Treasury> {
    try_get().and_then(Eth::treasury)
}

pub fn treasury() -> &'static Treasury {
    try_treasury().expect(
        "treasury not configured: set LOOP_ETH_TREASURY_KEY or add [eth.treasury] \
         to loop.toml (generate a key with `loop eth wallet new`)",
    )
}

pub fn balance(address: Address) -> Result<Wei, EthError> {
    get().balance(address)
}

pub async fn balance_async(address: Address) -> Result<Wei, EthError> {
    get().balance_async(address).await
}

pub fn gas_price() -> Result<Wei, EthError> {
    get().gas_price()
}

pub async fn gas_price_async() -> Result<Wei, EthError> {
    get().gas_price_async().await
}

pub fn block_number() -> Result<u64, EthError> {
    get().block_number()
}

pub async fn block_number_async() -> Result<u64, EthError> {
    get().block_number_async().await
}

pub fn transaction_count(address: Address) -> Result<u64, EthError> {
    get().transaction_count(address)
}

pub async fn transaction_count_async(address: Address) -> Result<u64, EthError> {
    get().transaction_count_async(address).await
}

/// New chain heads as an iterator, for `#[sse]`/`#[live]` handlers.
pub fn blocks() -> Result<super::events::BlockStream, EthError> {
    super::events::blocks()
}
