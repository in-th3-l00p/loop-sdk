use crate::{manifest, runtime};

pub fn wallet_new() {
    let (key, address) = lib::eth::generate_key();
    println!("address:     {address}");
    println!("private key: {key}");
    println!();
    println!("store the key somewhere safe (an env var, a secret manager) and");
    println!("point loop.toml at it:");
    println!();
    println!("  [eth.treasury]");
    println!("  key = \"env:TREASURY_KEY\"");
}

pub fn status(dir: &str) {
    if let Err(message) = status_inner(dir) {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn status_inner(dir: &str) -> Result<(), String> {
    let config = manifest::parse(dir)
        .map_err(|e| e.to_string())?
        .eth_config()?;

    runtime::block_on(async {
        let eth = lib::eth::Eth::connect(&config)
            .await
            .map_err(|e| e.to_string())?;
        println!("rpc:      {}", config.rpc_url);
        println!("chain id: {}", eth.chain_id());
        println!(
            "head:     block {}",
            eth.block_number_async().await.map_err(|e| e.to_string())?
        );
        match eth.treasury() {
            Some(treasury) => {
                let balance = eth
                    .balance_async(treasury.address())
                    .await
                    .map_err(|e| e.to_string())?;
                println!(
                    "treasury: {} ({} wei)",
                    treasury.address(),
                    balance.as_u256().to_dec()
                );
            }
            None => println!("treasury: not configured"),
        }
        Ok(())
    })
}
