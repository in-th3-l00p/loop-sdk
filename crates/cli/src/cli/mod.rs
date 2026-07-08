/* module that defines the CLI configuration of this app */

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(arg_required_else_help = true)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Init,
    Dev {
        #[arg(long)]
        port: Option<u16>,
        #[arg(long, default_value = ".")]
        dir: String,
    },
    Build {
        #[arg(long, default_value = ".")]
        dir: String,
    },
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
    Migration {
        #[command(subcommand)]
        command: MigrationCommands,
    },
    Eth {
        #[command(subcommand)]
        command: EthCommands,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    Devnet {
        #[command(subcommand)]
        command: DevnetCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum DevnetCommands {
    /// Register a local testnet with persisted chain state
    Create {
        /// Devnet name (default: "local")
        name: Option<String>,
        #[arg(long, default_value_t = 31337)]
        chain_id: u64,
        #[arg(long, default_value_t = 8545)]
        port: u16,
        /// Fork a live network so its contracts exist locally
        #[arg(long)]
        fork: Option<String>,
    },
    /// Run the devnet; chain state loads on start and dumps on shutdown
    Serve { name: Option<String> },
    /// List devnets with their status and persisted state size
    List,
    /// Permanently delete a devnet and its chain state
    Delete {
        name: String,
        /// Confirm the deletion; required since this is irreversible
        #[arg(long)]
        yes: bool,
    },
    /// Set an address's ether balance on the running devnet
    Fund {
        address: String,
        #[arg(long, default_value_t = 10)]
        eth: u64,
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthCommands {
    /// Secrets for the auth module (embedded wallet encryption)
    Secret {
        #[command(subcommand)]
        command: SecretCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum SecretCommands {
    /// Generate a 32-byte hex secret for [auth]; prints the loop.toml snippet
    New,
}

#[derive(Subcommand, Debug)]
pub enum DbCommands {
    /// Create the database (no-op for sqlite unless the file/directory is missing)
    Create {
        #[arg(long, default_value = ".")]
        dir: String,
    },
    /// Permanently delete the database
    Destroy {
        #[arg(long, default_value = ".")]
        dir: String,
        /// Confirm the deletion; required since this is irreversible
        #[arg(long)]
        yes: bool,
    },
    /// Apply pending migrations from migrations/
    Migrate {
        #[arg(long, default_value = ".")]
        dir: String,
    },
    /// Destroy, create, and migrate in one step
    Reset {
        #[arg(long, default_value = ".")]
        dir: String,
        /// Confirm the deletion; required since this is irreversible
        #[arg(long)]
        yes: bool,
    },
    /// Print the resolved connection URL
    Url {
        #[arg(long, default_value = ".")]
        dir: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum EthCommands {
    /// Wallet management for the app's treasury
    Wallet {
        #[command(subcommand)]
        command: WalletCommands,
    },
    /// Connect to the configured rpc and print chain id, head block and
    /// treasury balance
    Status {
        #[arg(long, default_value = ".")]
        dir: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum WalletCommands {
    /// Generate a keypair for [eth.treasury]; prints the address and key
    New,
}

#[derive(Subcommand, Debug)]
pub enum MigrationCommands {
    /// Scaffold a new migrations/<version>_<name>.sql file
    Create {
        name: String,
        #[arg(long, default_value = ".")]
        dir: String,
    },
    /// List migrations with their applied/pending state
    Status {
        #[arg(long, default_value = ".")]
        dir: String,
    },
}
