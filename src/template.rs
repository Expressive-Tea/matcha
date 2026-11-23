use include_dir::{include_dir, Dir};
use std::path::Path;

use crate::runtime::Runtime;

/// The `@green-tea/core` version `matcha new` writes into a scaffolded project,
/// substituted for `{{core_version}}` in the runtime overlays.
///
/// **Pinned exactly, on purpose.** Core is prerelease-only, and npm's semver
/// excludes a prerelease from any range whose comparators carry a different
/// `major.minor.patch` — so `^26.7.0-beta.0` never resolves past `26.7.0-beta.0`,
/// however many betas ship after it. A caret here is not a looser pin, it is a
/// pin to the oldest matching prerelease that nothing can move off. When core
/// reaches a stable major this becomes a range (`^27`) and the exactness stops
/// mattering; until then the `core-freshness` CI job fails when this falls
/// behind the `latest` dist-tag, which is what keeps it current.
pub const CORE_VERSION: &str = "26.11.0-rc.2";

pub static SHARED: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/shared");
pub static RUNTIME_DENO: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/runtimes/deno");
pub static RUNTIME_NODE: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/runtimes/node");
pub static RUNTIME_BUN: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/runtimes/bun");
pub static RUNTIME_EDGE: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/runtimes/edge");

fn overlay_for(runtime: Runtime) -> &'static Dir<'static> {
    match runtime {
        Runtime::Deno => &RUNTIME_DENO,
        Runtime::Node => &RUNTIME_NODE,
        Runtime::Bun => &RUNTIME_BUN,
        Runtime::Edge => &RUNTIME_EDGE,
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
                    .map(|s| render(s, name, CORE_VERSION).into_bytes())
                    .unwrap_or_else(|_| f.contents().to_vec());
                std::fs::write(out, contents)?;
            }
        }
    }
    Ok(())
}

fn render(template: &str, project_name: &str, core_version: &str) -> String {
    template
        .replace("{{project_name}}", project_name)
        .replace("{{core_version}}", core_version)
}

/// The files that tell a coding agent how this project and green-tea work:
/// `AGENTS.md` (read by Codex, OpenCode and others) and a `CLAUDE.md` that
/// imports it. `matcha new` writes them with the starter; `matcha agents`
/// writes them into a project that predates them.
pub const AGENT_FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// Renders one of [`AGENT_FILES`] for a project already on `core_version`.
pub fn agent_file(file: &str, project_name: &str, core_version: &str) -> Option<String> {
    let f = SHARED.get_file(file)?;
    Some(render(
        std::str::from_utf8(f.contents()).ok()?,
        project_name,
        core_version,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// The graph has to be importable without binding a port, which is why the
    /// scaffold composes it in `src/app.ts` and leaves `src/main.ts` holding
    /// only the runtime's serve call. `matcha graph` and `matcha explain`
    /// import the former; breaking the split breaks both.
    #[test]
    fn every_runtime_exports_the_app_from_its_own_module() {
        for rt in [Runtime::Node, Runtime::Deno, Runtime::Bun, Runtime::Edge] {
            let d = tempdir().unwrap();
            write_starter(d.path(), "my-api", rt).unwrap();

            let app = std::fs::read_to_string(d.path().join("src/app.ts")).unwrap();
            assert!(
                app.contains("export const app = createApp("),
                "{rt:?} app.ts"
            );

            let main = std::fs::read_to_string(d.path().join("src/main.ts")).unwrap();
            assert!(
                main.contains("import { app } from './app';"),
                "{rt:?} main.ts"
            );
            assert!(
                !main.contains("createApp("),
                "{rt:?} main.ts still composes the graph"
            );
        }
    }

    /// Core is prerelease-only, and a caret over a prerelease never resolves
    /// past its own `major.minor.patch` — `^26.7.0-beta.0` pinned every
    /// scaffold to 26.7.0-beta.0 while three later betas shipped. Nothing here
    /// may carry a range.
    #[test]
    fn no_runtime_scaffolds_a_caret_range_for_core() {
        for rt in [Runtime::Node, Runtime::Deno, Runtime::Bun, Runtime::Edge] {
            let d = tempdir().unwrap();
            write_starter(d.path(), "my-api", rt).unwrap();

            let manifest = ["package.json", "deno.json"]
                .iter()
                .map(|f| d.path().join(f))
                .find(|p| p.exists())
                .map(|p| std::fs::read_to_string(p).unwrap())
                .unwrap();
            let pin = manifest
                .lines()
                .find(|l| l.contains("@green-tea/core\":"))
                .unwrap_or_else(|| panic!("{rt:?} declares no core dependency"));
            assert!(pin.contains(CORE_VERSION), "{rt:?}: {pin}");
            assert!(!pin.contains('^'), "{rt:?} pins core with a caret: {pin}");
            assert!(!pin.contains('~'), "{rt:?} pins core with a tilde: {pin}");
        }
    }

    /// Every starter tells a coding agent where green-tea's rules are, with the
    /// version it was scaffolded on, and points it at the introspection
    /// commands rather than at guessing.
    #[test]
    fn every_runtime_scaffolds_the_agent_files() {
        for rt in [Runtime::Node, Runtime::Deno, Runtime::Bun, Runtime::Edge] {
            let d = tempdir().unwrap();
            write_starter(d.path(), "my-api", rt).unwrap();
            let agents = std::fs::read_to_string(d.path().join("AGENTS.md")).unwrap();
            assert!(agents.contains("# AGENTS.md — my-api"), "{rt:?}");
            assert!(agents.contains(CORE_VERSION), "{rt:?}");
            assert!(!agents.contains("{{"), "{rt:?} left a placeholder");
            assert!(agents.contains("matcha tokens --json"), "{rt:?}");
            assert!(
                agents.contains("node_modules/@green-tea/core/llms.txt"),
                "{rt:?}"
            );
            let claude = std::fs::read_to_string(d.path().join("CLAUDE.md")).unwrap();
            assert_eq!(claude.trim(), "@AGENTS.md", "{rt:?}");
        }
    }

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
        // serveDeno boots before it binds; Deno.serve(app.fetch) bound first and
        // left a port answering 500 when a provider failed.
        assert!(main.contains("await serveDeno(app"));
        assert!(!main.contains("Deno.serve("));
        assert!(!d.path().join("package.json").exists());

        let deno_json = std::fs::read_to_string(d.path().join("deno.json")).unwrap();
        assert!(deno_json.contains(&format!("jsr:@green-tea/core@{CORE_VERSION}")));
        assert!(!deno_json.contains("{{core_version}}"));
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

        assert!(pkg.contains(&format!("\"@green-tea/core\": \"{CORE_VERSION}\"")));
        assert!(!pkg.contains("{{core_version}}"));
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
        // Same reason as Deno: serveBun boots first.
        assert!(main.contains("await serveBun(app"));
        assert!(!main.contains("Bun.serve("));
        assert!(!d.path().join("deno.json").exists());
    }
}
