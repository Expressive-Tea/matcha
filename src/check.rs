use crate::runtime::{self, Runtime};
use std::path::Path;

fn check_command(rt: Runtime) -> (&'static str, Vec<&'static str>) {
    match rt {
        Runtime::Deno => ("deno", vec!["check", "src/app.module.ts"]),
        Runtime::Bun => ("bunx", vec!["tsc", "--noEmit"]),
        Runtime::Node | Runtime::Edge => ("npx", vec!["tsc", "--noEmit"]),
    }
}

pub fn run(dir: &Path) -> std::io::Result<bool> {
    let Some(rt) = runtime::detect(dir) else {
        return Ok(true);
    }; // nothing to check against
    let (prog, args) = check_command(rt);
    let status = std::process::Command::new(prog)
        .args(&args)
        .current_dir(dir)
        .status()?;
    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn deno_check_cmd() {
        assert_eq!(
            check_command(Runtime::Deno),
            ("deno", vec!["check", "src/app.module.ts"])
        );
    }

    #[test]
    fn node_uses_tsc_noemit() {
        assert_eq!(
            check_command(Runtime::Node),
            ("npx", vec!["tsc", "--noEmit"])
        );
    }

    #[test]
    fn bun_uses_bunx_tsc_noemit() {
        assert_eq!(
            check_command(Runtime::Bun),
            ("bunx", vec!["tsc", "--noEmit"])
        );
    }
}
