use std::io::{Error, ErrorKind};
use std::path::Path;

use crate::runtime::{self, Runtime};

fn command_for(rt: Runtime) -> (&'static str, Vec<&'static str>) {
    match rt {
        Runtime::Deno => ("deno", vec!["task", "dev"]),
        Runtime::Bun => ("bun", vec!["--watch", "run", "dev"]),
        Runtime::Node => ("npm", vec!["run", "dev"]),
    }
}

pub fn run(dir: &Path) -> std::io::Result<()> {
    let rt = runtime::detect(dir).ok_or_else(|| {
        Error::new(
            ErrorKind::NotFound,
            "no runtime detected (need deno.json, bun.lock, or package.json)",
        )
    })?;
    let (prog, args) = command_for(rt);
    println!("→ {prog} {}", args.join(" "));
    let status = std::process::Command::new(prog)
        .args(&args)
        .current_dir(dir)
        .status()?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn deno_uses_task_dev() {
        assert_eq!(command_for(Runtime::Deno), ("deno", vec!["task", "dev"]));
    }
    #[test]
    fn bun_watch() {
        assert_eq!(
            command_for(Runtime::Bun),
            ("bun", vec!["--watch", "run", "dev"])
        );
    }
    #[test]
    fn node_watch() {
        assert_eq!(command_for(Runtime::Node), ("npm", vec!["run", "dev"]));
    }
}
