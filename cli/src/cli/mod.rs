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
        #[arg(long, default_value_t = 3000)]
        port: u16,
        #[arg(long, default_value = ".")]
        dir: String,
    },
    Build {
        #[arg(long, default_value = ".")]
        dir: String,
    },
}
