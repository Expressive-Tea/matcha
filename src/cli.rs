use clap::builder::PossibleValuesParser;
use clap::{Parser, Subcommand};

use crate::{cmd_add, cmd_create};

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
        #[arg(value_parser = PossibleValuesParser::new(cmd_create::kinds()))]
        kind: String,
        /// Required for every kind but `plugin`, which asks for it
        name: Option<String>,
        #[arg(long)]
        check: bool,
        /// plugin: a package of its own, in DIR (default: the current directory, which must be empty)
        #[arg(long, num_args = 0..=1, value_name = "DIR")]
        package: Option<Option<String>>,
        /// plugin, in-app: folder relative to the project root (default: plugins)
        #[arg(long)]
        folder: Option<String>,
        /// plugin, package: the JSR scope
        #[arg(long)]
        scope: Option<String>,
        /// plugin, package: where it publishes (default: jsr)
        #[arg(long, value_parser = ["jsr", "both"])]
        registry: Option<String>,
        /// plugin, package, both: the npm name (default: @<scope>/green-tea-<slug>)
        #[arg(long)]
        npm_name: Option<String>,
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
    /// Print the structural OpenAPI 3.1 document for the route table
    Openapi {
        /// API title (default: core's own)
        #[arg(long)]
        title: Option<String>,
        /// API version (default: core's own)
        #[arg(long)]
        api_version: Option<String>,
        #[arg(long)]
        entry: Option<String>,
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
        #[arg(value_parser = PossibleValuesParser::new(cmd_add::names()))]
        capability: String,
        /// Which controller to edit (default: the only one in src/controllers)
        #[arg(long)]
        controller: Option<String>,
    },
}
