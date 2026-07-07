use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alloy::providers::{DynProvider, Provider, ProviderBuilder};

use super::config::Config;
use super::error::EthError;
use super::signer::Treasury;
use super::types::{Address, Wei};

/// The long-lived rpc client, established at startup and shared by every
/// handler. Blocking methods run on the server's runtime handle so they can
/// be called from handler threads, mirroring the database connection.
pub struct Eth {
    provider: DynProvider,
    handle: tokio::runtime::Handle,
    chain_id: u64,
    treasury: Option<Treasury>,
    poll_interval: Duration,
    sender_locks: Mutex<HashMap<Address, Arc<tokio::sync::Mutex<()>>>>,
}

impl std::fmt::Debug for Eth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Eth")
            .field("chain_id", &self.chain_id)
            .field("treasury", &self.treasury)
            .finish_non_exhaustive()
    }
}

impl Eth {
    pub async fn connect(config: &Config) -> Result<Eth, EthError> {
        let url = config
            .rpc_url
            .parse()
            .map_err(|e| EthError::Connect(format!("invalid rpc url {:?}: {e}", config.rpc_url)))?;
        let provider = ProviderBuilder::new().connect_http(url).erased();

        let chain_id = provider
            .get_chain_id()
            .await
            .map_err(|e| EthError::Connect(format!("{}: {e}", config.rpc_url)))?;
        if let Some(expected) = config.chain_id {
            if expected != chain_id {
                return Err(EthError::Connect(format!(
                    "chain id mismatch: rpc reports {chain_id}, config expects {expected}"
                )));
            }
        }

        let treasury = config
            .treasury_key
            .as_deref()
            .map(Treasury::from_key)
            .transpose()?;

        Ok(Eth {
            provider,
            handle: tokio::runtime::Handle::current(),
            chain_id,
            treasury,
            poll_interval: config.poll_interval,
            sender_locks: Mutex::new(HashMap::new()),
        })
    }

    /// Serializes transaction sends per sender address so nonce selection
    /// stays race-free within this server instance.
    pub(crate) async fn sender_lock(&self, sender: Address) -> tokio::sync::OwnedMutexGuard<()> {
        let mutex = {
            let mut locks = self.sender_locks.lock().expect("sender lock poisoned");
            locks.entry(sender).or_default().clone()
        };
        mutex.lock_owned().await
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub fn treasury(&self) -> Option<&Treasury> {
        self.treasury.as_ref()
    }

    pub fn poll_interval(&self) -> Duration {
        self.poll_interval
    }

    pub(crate) fn provider(&self) -> &DynProvider {
        &self.provider
    }

    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }

    pub async fn balance_async(&self, address: Address) -> Result<Wei, EthError> {
        let balance = self
            .provider
            .get_balance(address.0)
            .await
            .map_err(rpc_error)?;
        Ok(Wei(super::types::U256(balance)))
    }

    pub fn balance(&self, address: Address) -> Result<Wei, EthError> {
        self.block_on(self.balance_async(address))
    }

    pub async fn gas_price_async(&self) -> Result<Wei, EthError> {
        let price = self.provider.get_gas_price().await.map_err(rpc_error)?;
        Ok(Wei(super::types::U256::from(price)))
    }

    pub fn gas_price(&self) -> Result<Wei, EthError> {
        self.block_on(self.gas_price_async())
    }

    pub async fn block_number_async(&self) -> Result<u64, EthError> {
        self.provider.get_block_number().await.map_err(rpc_error)
    }

    pub fn block_number(&self) -> Result<u64, EthError> {
        self.block_on(self.block_number_async())
    }

    pub async fn transaction_count_async(&self, address: Address) -> Result<u64, EthError> {
        self.provider
            .get_transaction_count(address.0)
            .await
            .map_err(rpc_error)
    }

    pub fn transaction_count(&self, address: Address) -> Result<u64, EthError> {
        self.block_on(self.transaction_count_async(address))
    }
}

pub(crate) fn rpc_error(e: impl std::fmt::Display) -> EthError {
    EthError::Rpc(e.to_string())
}
