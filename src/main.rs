mod cli;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::New { .. } => eprintln!("new: not yet implemented"),
        Command::Run => eprintln!("run: not yet implemented"),
        Command::Create { .. } => eprintln!("create: not yet implemented"),
        Command::Add { .. } => eprintln!("add: not yet implemented"),
    }
}
