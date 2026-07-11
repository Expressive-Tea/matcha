mod cli;
mod cmd_new;
mod runtime;
mod template;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::New { name, template_url } => {
            if let Err(e) = cmd_new::run(&name, template_url.as_deref()) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::Run => eprintln!("run: not yet implemented"),
        Command::Create { .. } => eprintln!("create: not yet implemented"),
        Command::Add { .. } => eprintln!("add: not yet implemented"),
    }
}
