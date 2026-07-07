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
