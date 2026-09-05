use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "matcha", version, about = "green-tea CLI")]
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
        #[arg(long, default_value = "node", value_parser = ["node", "deno", "bun", "edge"])]
        runtime: String,
    },
    /// Detect the runtime and run the project in watch mode
    Run,
    /// Generate a piece and auto-wire it into the module
    Create {
        #[arg(value_parser = ["module", "controller", "step", "provider"])]
        kind: String,
        name: String,
        #[arg(long)]
        check: bool,
    },
    /// Check the project for the misconfigurations that fail confusingly
    Doctor,
    /// Print the dependency graph (Mermaid, DOT, or JSON)
    Graph {
        #[arg(long, default_value = "mermaid", value_parser = ["mermaid", "dot", "json"])]
        format: String,
        /// Entry file exporting `app` (default: src/app.ts, then src/main.ts)
        #[arg(long)]
        entry: Option<String>,
        /// Write to this file instead of stdout
        #[arg(long)]
        out: Option<String>,
    },
    /// Explain one route's chain, in execution order
    Explain {
        /// The route pattern as declared, e.g. /users/:id
        route: String,
        #[arg(long)]
        entry: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
    /// Add a capability to a controller
    Add {
        #[arg(value_parser = ["sse", "stream", "buffer", "ws", "upload"])]
        capability: String,
        /// Which controller to edit (default: the only one in src/controllers)
        #[arg(long)]
        controller: Option<String>,
    },
}
