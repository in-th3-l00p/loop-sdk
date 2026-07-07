mod cli;
mod commands;
mod manifest;

use clap::Parser;
use cli::{Args, Commands};

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Init => commands::init::run(),
        Commands::Dev { port, dir } => commands::dev::run(&dir, port),
        Commands::Build { dir } => commands::build::run(&dir),
    }
}
