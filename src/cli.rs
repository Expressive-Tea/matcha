use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "matcha", about = "green-tea CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scaffold a new project
    New {
        name: String,
        #[arg(long)]
        template_url: Option<String>,
        #[arg(long, default_value = "deno", value_parser = ["node", "deno", "bun"])]
        runtime: String,
    },
    /// Detect the runtime and run the project in watch mode
    Run,
    /// Generate a piece and auto-wire it into the module
    Create {
        #[arg(value_parser = ["module", "controller", "step"])]
        kind: String,
        name: String,
        #[arg(long)]
        check: bool,
    },
    /// Add a capability to a controller
    Add {
        #[arg(value_parser = ["sse", "stream", "buffer"])]
        capability: String,
    },
}
