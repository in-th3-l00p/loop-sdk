use std::time::Duration;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    /// Verified against `eth_chainId` at startup when set.
    pub chain_id: Option<u64>,
    /// 0x-hex secp256k1 private key for the server-owned treasury wallet.
    pub treasury_key: Option<String>,
    /// How often event and block streams poll the rpc.
    pub poll_interval: Duration,
}

impl Config {
    pub fn from_rpc(url: impl Into<String>) -> Config {
        Config {
            rpc_url: url.into(),
            chain_id: None,
            treasury_key: None,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }

    /// Reads `LOOP_ETH_RPC_URL` (required), `LOOP_ETH_CHAIN_ID`,
    /// `LOOP_ETH_TREASURY_KEY` and `LOOP_ETH_POLL_MS`. Returns `None` when no
    /// rpc url is set so startup stays a no-op for apps without eth.
    pub fn from_env() -> Option<Config> {
        let rpc_url = std::env::var("LOOP_ETH_RPC_URL").ok()?;
        let chain_id = std::env::var("LOOP_ETH_CHAIN_ID")
            .ok()
            .and_then(|id| id.parse().ok());
        let treasury_key = std::env::var("LOOP_ETH_TREASURY_KEY").ok();
        let poll_interval = std::env::var("LOOP_ETH_POLL_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_POLL_INTERVAL);
        Some(Config {
            rpc_url,
            chain_id,
            treasury_key,
            poll_interval,
        })
    }
}
