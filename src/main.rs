mod check;
mod cli;
mod cmd_add;
mod cmd_create;
mod cmd_graph;
mod cmd_new;
mod cmd_run;
mod entry;
mod runtime;
mod template;
mod wire;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::New {
            name,
            template_url,
            runtime,
        } => {
            if let Err(e) = cmd_new::run(&name, template_url.as_deref(), &runtime) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::Run => {
            if let Err(e) = cmd_run::run(std::path::Path::new(".")) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::Create { kind, name, check } => {
            if let Err(e) = cmd_create::run(&kind, &name, check) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::Graph { format, entry, out } => {
            if let Err(e) = cmd_graph::graph(
                std::path::Path::new("."),
                &format,
                entry.as_deref(),
                out.as_deref(),
            ) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::Explain { route, entry, out } => {
            if let Err(e) = cmd_graph::explain(
                std::path::Path::new("."),
                &route,
                entry.as_deref(),
                out.as_deref(),
            ) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Command::Add { capability } => {
            if let Err(e) = cmd_add::run(&capability) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}
