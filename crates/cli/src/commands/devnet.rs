/* named local testnets with persisted chain state, wrapping anvil
(foundry's node). the CLI owns a directory per devnet under
~/.loop/devnets/<name>/ holding its config and its state file — stop the
node and the chain survives; serve again and it picks up where it left
off. `fund` drops ether on any address via anvil_setBalance, so apps'
embedded wallets can pay for gas without touching the dev mnemonic */

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Devnet {
    pub name: String,
    pub chain_id: u64,
    pub port: u16,
    pub fork: Option<String>,
}

const DEFAULT_NAME: &str = "local";

/// The ten anvil/hardhat dev accounts (mnemonic "test test … junk"),
/// each funded with 10000 ether. Publicly-known keys — never mainnet.
const DEV_ACCOUNTS: &[(&str, &str)] = &[
    (
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    ),
    (
        "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
        "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
    ),
    (
        "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
        "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
    ),
    (
        "0x90F79bf6EB2c4f870365E785982E1f101E93b906",
        "0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6",
    ),
    (
        "0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65",
        "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
    ),
];

fn run(command: impl FnOnce() -> Result<(), String>) {
    if let Err(message) = command() {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn root() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "cannot resolve $HOME".to_string())?;
    Ok(PathBuf::from(home).join(".loop").join("devnets"))
}

fn dir_of(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(format!(
            "devnet names are alphanumeric-with-dashes, got {name:?}"
        ));
    }
    Ok(root()?.join(name))
}

fn load(name: &str) -> Result<Devnet, String> {
    let path = dir_of(name)?.join("devnet.toml");
    let text = std::fs::read_to_string(&path).map_err(|_| {
        format!("no devnet named {name:?} — create it with `loop devnet create {name}`")
    })?;
    toml::from_str(&text).map_err(|e| format!("corrupt devnet config: {e}"))
}

fn require_anvil() -> Result<(), String> {
    match Command::new("anvil").arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err("anvil not found — install foundry: \
                  curl -L https://foundry.paradigm.xyz | bash && foundryup"
            .to_string()),
    }
}

pub fn create(name: Option<String>, chain_id: u64, port: u16, fork: Option<String>) {
    run(|| {
        require_anvil()?;
        let name = name.unwrap_or_else(|| DEFAULT_NAME.to_string());
        let dir = dir_of(&name)?;
        if dir.join("devnet.toml").exists() {
            return Err(format!("devnet {name:?} already exists"));
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let devnet = Devnet {
            name: name.clone(),
            chain_id,
            port,
            fork,
        };
        let config = toml::to_string_pretty(&devnet).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("devnet.toml"), config).map_err(|e| e.to_string())?;

        println!("created devnet {name:?}");
        println!("  chain id: {chain_id}");
        println!("  rpc:      http://127.0.0.1:{port}");
        if let Some(fork) = &devnet.fork {
            println!("  forking:  {fork}");
        }
        println!("  state:    {}", dir.join("state.json").display());
        println!();
        println!("funded dev accounts (public keys — never use on a real network):");
        for (address, key) in DEV_ACCOUNTS {
            println!("  {address}  {key}");
        }
        println!();
        println!("start it with `loop devnet serve {name}`, point apps at");
        println!("ETH_RPC_URL=http://127.0.0.1:{port}");
        Ok(())
    })
}

pub fn serve(name: Option<String>) {
    run(|| {
        require_anvil()?;
        let name = name.unwrap_or_else(|| DEFAULT_NAME.to_string());
        let devnet = load(&name)?;
        let dir = dir_of(&name)?;
        let state = dir.join("state.json");

        println!(
            "devnet {name:?} on http://127.0.0.1:{} (chain {}) — state persists to {}",
            devnet.port,
            devnet.chain_id,
            state.display()
        );

        let mut command = Command::new("anvil");
        command
            .arg("--chain-id")
            .arg(devnet.chain_id.to_string())
            .arg("--port")
            .arg(devnet.port.to_string())
            // --state loads the file when present and dumps on shutdown
            .arg("--state")
            .arg(&state);
        if let Some(fork) = &devnet.fork {
            command.arg("--fork-url").arg(fork);
        }
        let status = command
            .status()
            .map_err(|e| format!("failed to run anvil: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("anvil exited with {status}"))
        }
    })
}

pub fn list() {
    run(|| {
        let root = root()?;
        let Ok(entries) = std::fs::read_dir(&root) else {
            println!("no devnets yet — `loop devnet create` makes one");
            return Ok(());
        };
        let mut found = false;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(devnet) = load(&name) else { continue };
            found = true;
            let state = entry.path().join("state.json");
            let persisted = match std::fs::metadata(&state) {
                Ok(meta) => format!("{} KiB of state", meta.len().div_ceil(1024)),
                Err(_) => "fresh".to_string(),
            };
            let status = if listening(devnet.port) {
                "running"
            } else {
                "stopped"
            };
            let fork = devnet
                .fork
                .as_deref()
                .map(|f| format!(", forking {f}"))
                .unwrap_or_default();
            println!(
                "{name}  chain {}  port {}  {status}  {persisted}{fork}",
                devnet.chain_id, devnet.port
            );
        }
        if !found {
            println!("no devnets yet — `loop devnet create` makes one");
        }
        Ok(())
    })
}

pub fn delete(name: &str, yes: bool) {
    run(|| {
        let devnet = load(name)?;
        if listening(devnet.port) {
            return Err(format!("devnet {name:?} is running — stop it first"));
        }
        if !yes {
            return Err(format!(
                "deleting {name:?} destroys its chain state; pass --yes to confirm"
            ));
        }
        std::fs::remove_dir_all(dir_of(name)?).map_err(|e| e.to_string())?;
        println!("deleted devnet {name:?} and its state");
        Ok(())
    })
}

pub fn fund(name: Option<String>, address: &str, eth: u64) {
    run(|| {
        let name = name.unwrap_or_else(|| DEFAULT_NAME.to_string());
        let devnet = load(&name)?;
        if !listening(devnet.port) {
            return Err(format!(
                "devnet {name:?} is not running — `loop devnet serve {name}` first"
            ));
        }
        let address = address
            .parse::<lib::eth::Address>()
            .map_err(|e| e.to_string())?;
        let wei = format!("0x{:x}", u128::from(eth) * 1_000_000_000_000_000_000);
        rpc(
            devnet.port,
            "anvil_setBalance",
            &format!("[\"{address}\", \"{wei}\"]"),
        )?;
        println!("funded {address} with {eth} ETH on devnet {name:?}");
        Ok(())
    })
}

fn listening(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &([127, 0, 0, 1], port).into(),
        std::time::Duration::from_millis(300),
    )
    .is_ok()
}

/// A minimal JSON-RPC call to the local node — hand-rolled HTTP/1.1 over a
/// TcpStream, so the CLI needs no HTTP client dependency.
fn rpc(port: u16, method: &str, params: &str) -> Result<String, String> {
    let body = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"{method}\",\"params\":{params}}}");
    let request = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("cannot reach the devnet: {e}"))?;
    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    let payload = response
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .to_string();
    if payload.contains("\"error\"") {
        return Err(format!("rpc {method} failed: {payload}"));
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devnet_configs_roundtrip_through_toml() {
        let devnet = Devnet {
            name: "demo".into(),
            chain_id: 31337,
            port: 8545,
            fork: Some("https://rpc.example".into()),
        };
        let text = toml::to_string_pretty(&devnet).unwrap();
        let back: Devnet = toml::from_str(&text).unwrap();
        assert_eq!(back.name, "demo");
        assert_eq!(back.chain_id, 31337);
        assert_eq!(back.fork.as_deref(), Some("https://rpc.example"));
    }

    #[test]
    fn names_are_constrained_to_safe_paths() {
        assert!(dir_of("local").is_ok());
        assert!(dir_of("my-net-2").is_ok());
        for bad in ["", "../up", "a/b", "dot.name", "sp ace"] {
            assert!(dir_of(bad).is_err(), "accepted {bad:?}");
        }
    }
}
