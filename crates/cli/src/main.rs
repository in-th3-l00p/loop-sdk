mod cli;
mod commands;
mod manifest;
mod migrations;
mod runtime;

use clap::Parser;
use cli::{
    Args, AuthCommands, Commands, DbCommands, DevnetCommands, EthCommands, MigrationCommands,
    SecretCommands, WalletCommands,
};

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Init => commands::init::run(),
        Commands::Dev { port, dir } => commands::dev::run(&dir, port),
        Commands::Build { dir } => commands::build::run(&dir),
        Commands::Db { command } => match command {
            DbCommands::Create { dir } => commands::db::create(&dir),
            DbCommands::Destroy { dir, yes } => commands::db::destroy(&dir, yes),
            DbCommands::Migrate { dir } => commands::db::migrate(&dir),
            DbCommands::Reset { dir, yes } => commands::db::reset(&dir, yes),
            DbCommands::Url { dir } => commands::db::url(&dir),
        },
        Commands::Migration { command } => match command {
            MigrationCommands::Create { dir, name } => commands::migration::create(&dir, &name),
            MigrationCommands::Status { dir } => commands::migration::status(&dir),
        },
        Commands::Eth { command } => match command {
            EthCommands::Wallet { command } => match command {
                WalletCommands::New => commands::eth::wallet_new(),
            },
            EthCommands::Status { dir } => commands::eth::status(&dir),
        },
        Commands::Auth { command } => match command {
            AuthCommands::Secret { command } => match command {
                SecretCommands::New => commands::auth::secret_new(),
            },
        },
        Commands::Devnet { command } => match command {
            DevnetCommands::Create {
                name,
                chain_id,
                port,
                fork,
            } => commands::devnet::create(name, chain_id, port, fork),
            DevnetCommands::Serve { name } => commands::devnet::serve(name),
            DevnetCommands::List => commands::devnet::list(),
            DevnetCommands::Delete { name, yes } => commands::devnet::delete(&name, yes),
            DevnetCommands::Fund { address, eth, name } => {
                commands::devnet::fund(name, &address, eth)
            }
        },
    }
}
