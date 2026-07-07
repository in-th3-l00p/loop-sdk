mod cli;
mod commands;
mod manifest;
mod migrations;
mod runtime;

use clap::Parser;
use cli::{Args, Commands, DbCommands, EthCommands, MigrationCommands, WalletCommands};

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
    }
}
