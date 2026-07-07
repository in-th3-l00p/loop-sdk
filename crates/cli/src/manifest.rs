/* interpretation of a loop project's manifest (loop.toml) */

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub dev: Dev,
    pub database: Option<Database>,
    pub eth: Option<Eth>,
}

#[derive(Deserialize, Default)]
pub struct Dev {
    pub port: Option<u16>,
}

#[derive(Deserialize)]
pub struct Database {
    pub url: Option<String>,
}

#[derive(Deserialize)]
pub struct Eth {
    pub rpc: Option<String>,
    pub chain_id: Option<u64>,
    pub poll_interval_ms: Option<u64>,
    pub treasury: Option<EthTreasury>,
}

#[derive(Deserialize)]
pub struct EthTreasury {
    pub key: Option<String>,
}

/// Manifest values may point at the environment with an `env:VAR` prefix, so
/// secrets like rpc keys stay out of loop.toml.
fn resolve(value: &str) -> Result<String, String> {
    match value.strip_prefix("env:") {
        Some(var) => std::env::var(var)
            .map_err(|_| format!("loop.toml references env:{var} but {var} is not set")),
        None => Ok(value.to_string()),
    }
}

impl Manifest {
    /// The connection URL `loop dev` exports as `LOOP_DB_URL`. Only projects
    /// with a `[database]` section get one; a shell-level `LOOP_DB_URL`
    /// overrides the manifest, and the fallback is a project-named sqlite
    /// file, so `[database]` alone is enough for local dev.
    pub fn database_url(&self) -> Result<Option<String>, String> {
        let Some(database) = self.database.as_ref() else {
            return Ok(None);
        };
        let url = match std::env::var("LOOP_DB_URL") {
            Ok(url) => url,
            Err(_) => match &database.url {
                Some(url) => resolve(url)?,
                None => format!("sqlite:{}.db", self.name),
            },
        };
        Ok(Some(url))
    }

    /// The connection config `loop db`/`loop migration` commands act on.
    pub fn database_config(&self) -> Result<lib::database::Config, String> {
        self.database_url()?
            .map(lib::database::Config::from_url)
            .ok_or_else(|| "no database configured — add a [database] section to loop.toml".to_string())
    }

    /// The `LOOP_ETH_*` variables `loop dev` exports for projects with an
    /// `[eth]` section. Shell-level variables override the manifest; a
    /// missing rpc url is an error rather than a silently eth-less server.
    pub fn eth_env(&self) -> Result<Vec<(&'static str, String)>, String> {
        let Some(eth) = self.eth.as_ref() else {
            return Ok(Vec::new());
        };

        let rpc = match std::env::var("LOOP_ETH_RPC_URL") {
            Ok(url) => url,
            Err(_) => match &eth.rpc {
                Some(rpc) => resolve(rpc)?,
                None => {
                    return Err(
                        "[eth] needs an rpc url — set rpc = \"https://…\" or rpc = \"env:VAR\" \
                         in loop.toml"
                            .to_string(),
                    );
                }
            },
        };
        let mut vars = vec![("LOOP_ETH_RPC_URL", rpc)];

        if let Some(chain_id) = std::env::var("LOOP_ETH_CHAIN_ID")
            .ok()
            .or_else(|| eth.chain_id.map(|id| id.to_string()))
        {
            vars.push(("LOOP_ETH_CHAIN_ID", chain_id));
        }
        if let Some(poll_ms) = std::env::var("LOOP_ETH_POLL_MS")
            .ok()
            .or_else(|| eth.poll_interval_ms.map(|ms| ms.to_string()))
        {
            vars.push(("LOOP_ETH_POLL_MS", poll_ms));
        }

        let manifest_key = eth.treasury.as_ref().and_then(|t| t.key.as_deref());
        if let Some(key) = match std::env::var("LOOP_ETH_TREASURY_KEY") {
            Ok(key) => Some(key),
            Err(_) => manifest_key.map(resolve).transpose()?,
        } {
            vars.push(("LOOP_ETH_TREASURY_KEY", key));
        }

        Ok(vars)
    }

    /// The client config `loop eth` commands act on.
    pub fn eth_config(&self) -> Result<lib::eth::Config, String> {
        let vars = self.eth_env()?;
        if self.eth.is_none() {
            return Err("no eth configured — add an [eth] section to loop.toml".to_string());
        }
        let value = |name: &str| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone())
        };
        let mut config =
            lib::eth::Config::from_rpc(value("LOOP_ETH_RPC_URL").expect("checked above"));
        config.chain_id = value("LOOP_ETH_CHAIN_ID").and_then(|id| id.parse().ok());
        config.treasury_key = value("LOOP_ETH_TREASURY_KEY");
        if let Some(ms) = value("LOOP_ETH_POLL_MS").and_then(|ms| ms.parse().ok()) {
            config.poll_interval = std::time::Duration::from_millis(ms);
        }
        Ok(config)
    }
}

#[derive(Debug)]
pub enum ManifestError {
    Io(PathBuf, std::io::Error),
    Invalid(toml::de::Error),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Io(path, e) => {
                write!(f, "{}: {e} (is this a loop project?)", path.display())
            }
            ManifestError::Invalid(e) => write!(f, "invalid loop.toml: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

pub fn parse(dir: impl AsRef<Path>) -> Result<Manifest, ManifestError> {
    let path = dir.as_ref().join("loop.toml");
    let text = fs::read_to_string(&path).map_err(|e| ManifestError::Io(path, e))?;
    toml::from_str(&text).map_err(ManifestError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_dev_port() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("loop.toml"),
            "name = \"my-api\"\n\n[dev]\nport = 4000\n",
        )
        .unwrap();

        let manifest = parse(dir.path()).unwrap();
        assert_eq!(manifest.name, "my-api");
        assert_eq!(manifest.dev.port, Some(4000));
    }

    #[test]
    fn dev_section_is_optional() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("loop.toml"), "name = \"my-api\"\n").unwrap();

        let manifest = parse(dir.path()).unwrap();
        assert_eq!(manifest.dev.port, None);
    }

    #[test]
    fn fails_for_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(parse(dir.path()), Err(ManifestError::Io(_, _))));
    }

    #[test]
    fn database_url_needs_a_database_section() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("loop.toml"), "name = \"my-api\"\n").unwrap();
        assert_eq!(parse(dir.path()).unwrap().database_url().unwrap(), None);
    }

    #[test]
    fn database_url_defaults_to_a_project_named_sqlite_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("loop.toml"), "name = \"my-api\"\n\n[database]\n").unwrap();
        assert_eq!(
            parse(dir.path()).unwrap().database_url().unwrap(),
            Some("sqlite:my-api.db".into())
        );
    }

    #[test]
    fn database_url_comes_from_the_manifest_when_set() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("loop.toml"),
            "name = \"my-api\"\n\n[database]\nurl = \"postgres://localhost/shop\"\n",
        )
        .unwrap();
        assert_eq!(
            parse(dir.path()).unwrap().database_url().unwrap(),
            Some("postgres://localhost/shop".into())
        );
    }

    #[test]
    fn eth_env_is_empty_without_an_eth_section() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("loop.toml"), "name = \"my-api\"\n").unwrap();
        assert_eq!(parse(dir.path()).unwrap().eth_env().unwrap(), Vec::new());
    }

    #[test]
    fn eth_env_exports_the_configured_values() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("loop.toml"),
            "name = \"my-api\"\n\n[eth]\nrpc = \"https://rpc.example\"\nchain_id = 1\npoll_interval_ms = 500\n",
        )
        .unwrap();
        assert_eq!(
            parse(dir.path()).unwrap().eth_env().unwrap(),
            vec![
                ("LOOP_ETH_RPC_URL", "https://rpc.example".to_string()),
                ("LOOP_ETH_CHAIN_ID", "1".to_string()),
                ("LOOP_ETH_POLL_MS", "500".to_string()),
            ]
        );
    }

    #[test]
    fn eth_section_without_rpc_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("loop.toml"),
            "name = \"my-api\"\n\n[eth]\nchain_id = 1\n",
        )
        .unwrap();
        let error = parse(dir.path()).unwrap().eth_env().unwrap_err();
        assert!(error.contains("rpc url"), "unexpected error: {error}");
    }

    #[test]
    fn env_indirection_resolves_and_reports_missing_vars() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("loop.toml"),
            "name = \"my-api\"\n\n[eth]\nrpc = \"env:LOOP_TEST_ETH_RPC_INDIRECT\"\n\n[eth.treasury]\nkey = \"env:LOOP_TEST_ETH_KEY_UNSET\"\n",
        )
        .unwrap();
        let manifest = parse(dir.path()).unwrap();

        let error = manifest.eth_env().unwrap_err();
        assert!(
            error.contains("LOOP_TEST_ETH_RPC_INDIRECT"),
            "unexpected error: {error}"
        );

        unsafe { std::env::set_var("LOOP_TEST_ETH_RPC_INDIRECT", "https://indirect.example") };
        let error = manifest.eth_env().unwrap_err();
        assert!(
            error.contains("LOOP_TEST_ETH_KEY_UNSET"),
            "unexpected error: {error}"
        );

        unsafe { std::env::set_var("LOOP_TEST_ETH_KEY_UNSET", "0xkey") };
        assert_eq!(
            manifest.eth_env().unwrap(),
            vec![
                ("LOOP_ETH_RPC_URL", "https://indirect.example".to_string()),
                ("LOOP_ETH_TREASURY_KEY", "0xkey".to_string()),
            ]
        );
    }
}
