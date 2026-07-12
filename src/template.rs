use include_dir::{include_dir, Dir};
use std::path::Path;

use crate::runtime::Runtime;

pub static SHARED: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/shared");
pub static RUNTIME_DENO: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/runtimes/deno");
pub static RUNTIME_NODE: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/runtimes/node");
pub static RUNTIME_BUN: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/runtimes/bun");

fn overlay_for(runtime: Runtime) -> &'static Dir<'static> {
    match runtime {
        Runtime::Deno => &RUNTIME_DENO,
        Runtime::Node => &RUNTIME_NODE,
        Runtime::Bun => &RUNTIME_BUN,
    }
}

pub fn write_starter(dest: &Path, project_name: &str, runtime: Runtime) -> std::io::Result<()> {
    write_dir(&SHARED, dest, project_name)?;
    write_dir(overlay_for(runtime), dest, project_name)
}

fn write_dir(dir: &Dir, dest: &Path, name: &str) -> std::io::Result<()> {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(d) => {
                let sub = dest.join(d.path().file_name().unwrap());
                std::fs::create_dir_all(&sub)?;
                write_dir(d, &sub, name)?;
            }
            include_dir::DirEntry::File(f) => {
                let out = dest.join(f.path().file_name().unwrap());
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let contents = std::str::from_utf8(f.contents())
                    .map(|s| s.replace("{{project_name}}", name).into_bytes())
                    .unwrap_or_else(|_| f.contents().to_vec());
                std::fs::write(out, contents)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn deno_writes_shared_and_deno_overlay() {
        let d = tempdir().unwrap();
        write_starter(d.path(), "my-api", Runtime::Deno).unwrap();

        let html = std::fs::read_to_string(d.path().join("public/index.html")).unwrap();
        assert!(html.contains("<title>my-api</title>"));
        assert!(!html.contains("{{project_name}}"));
        assert!(d.path().join("src/controllers/home.controller.ts").exists());
        let module = std::fs::read_to_string(d.path().join("src/app.module.ts")).unwrap();
        assert!(module.contains("mountpoint"));

        assert!(d.path().join("deno.json").exists());
        assert!(d.path().join("matcha.toml").exists());
        let main = std::fs::read_to_string(d.path().join("src/main.ts")).unwrap();
        assert!(main.contains("Deno.serve"));
        assert!(!d.path().join("package.json").exists());
    }

    #[test]
    fn node_writes_shared_and_node_overlay() {
        let d = tempdir().unwrap();
        write_starter(d.path(), "my-api", Runtime::Node).unwrap();

        assert!(d.path().join("public/index.html").exists());
        assert!(d.path().join("src/app.module.ts").exists());

        let pkg = std::fs::read_to_string(d.path().join("package.json")).unwrap();
        assert!(pkg.contains("\"name\": \"my-api\""));
        assert!(!pkg.contains("{{project_name}}"));
        assert!(d.path().join("tsconfig.json").exists());
        let main = std::fs::read_to_string(d.path().join("src/main.ts")).unwrap();
        assert!(main.contains("app.listen"));
        assert!(!d.path().join("deno.json").exists());
    }

    #[test]
    fn bun_writes_shared_and_bun_overlay() {
        let d = tempdir().unwrap();
        write_starter(d.path(), "my-api", Runtime::Bun).unwrap();

        assert!(d.path().join("public/index.html").exists());
        assert!(d.path().join("src/app.module.ts").exists());

        let pkg = std::fs::read_to_string(d.path().join("package.json")).unwrap();
        assert!(pkg.contains("\"name\": \"my-api\""));
        assert!(!pkg.contains("{{project_name}}"));
        let main = std::fs::read_to_string(d.path().join("src/main.ts")).unwrap();
        assert!(main.contains("Bun.serve"));
        assert!(!d.path().join("deno.json").exists());
    }
}
