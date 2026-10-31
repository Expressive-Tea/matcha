//! `matcha agents` — write `AGENTS.md` and `CLAUDE.md` into a project that was
//! scaffolded before matcha wrote them, so a coding agent opening it finds
//! green-tea's rules instead of applying Express habits.

use std::path::Path;

use crate::template::{self, AGENT_FILES, CORE_VERSION};

pub fn run(dir: &Path, force: bool) -> std::io::Result<()> {
    let name = project_name(dir);
    let version = core_version(dir).unwrap_or_else(|| CORE_VERSION.to_string());

    for file in AGENT_FILES {
        let path = dir.join(file);
        if path.exists() && !force {
            println!("· {file} exists, left as is (--force to overwrite)");
            continue;
        }
        let contents = template::agent_file(file, &name, &version)
            .expect("agent files are embedded in the binary");
        std::fs::write(&path, contents)?;
        println!("✓ {file}");
    }
    Ok(())
}

/// `package.json`'s `name`, else the directory's.
fn project_name(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|pkg| field(&pkg, "\"name\""))
        .or_else(|| {
            std::fs::canonicalize(dir)
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        })
        .unwrap_or_else(|| "this project".to_string())
}

/// The core version the project actually depends on, so the file names the
/// version whose rules apply: `package.json` (npm, Bun, edge) or `deno.json`'s
/// `jsr:@green-tea/core@…` import.
fn core_version(dir: &Path) -> Option<String> {
    if let Ok(pkg) = std::fs::read_to_string(dir.join("package.json")) {
        if let Some(v) = field(&pkg, "\"@green-tea/core\"") {
            return Some(v.trim_start_matches(['^', '~', '=']).to_string());
        }
    }
    let deno = std::fs::read_to_string(dir.join("deno.json")).ok()?;
    let at = deno.find("@green-tea/core@")? + "@green-tea/core@".len();
    Some(
        deno[at..]
            .chars()
            .take_while(|c| !matches!(c, '"' | '/'))
            .collect(),
    )
}

/// The string value after `key:` in a JSON text. Enough for the two flat
/// fields read here, without a JSON parse of a file that may not be valid.
fn field(json: &str, key: &str) -> Option<String> {
    let rest = &json[json.find(key)? + key.len()..];
    let rest = rest
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_both_files_with_the_projects_own_core_version() {
        let d = tempdir().unwrap();
        std::fs::write(
            d.path().join("package.json"),
            r#"{ "name": "shop", "dependencies": { "@green-tea/core": "26.9.0-beta.1" } }"#,
        )
        .unwrap();
        run(d.path(), false).unwrap();
        let agents = std::fs::read_to_string(d.path().join("AGENTS.md")).unwrap();
        assert!(agents.contains("# AGENTS.md — shop"));
        assert!(agents.contains("26.9.0-beta.1"));
        assert_eq!(
            std::fs::read_to_string(d.path().join("CLAUDE.md"))
                .unwrap()
                .trim(),
            "@AGENTS.md"
        );
    }

    #[test]
    fn leaves_an_existing_file_alone_unless_forced() {
        let d = tempdir().unwrap();
        std::fs::write(d.path().join("AGENTS.md"), "mine").unwrap();
        run(d.path(), false).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join("AGENTS.md")).unwrap(),
            "mine"
        );
        run(d.path(), true).unwrap();
        assert_ne!(
            std::fs::read_to_string(d.path().join("AGENTS.md")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn reads_the_version_from_a_deno_import() {
        let d = tempdir().unwrap();
        std::fs::write(
            d.path().join("deno.json"),
            r#"{ "imports": { "@green-tea/core": "jsr:@green-tea/core@26.9.0-beta.2" } }"#,
        )
        .unwrap();
        assert_eq!(core_version(d.path()).as_deref(), Some("26.9.0-beta.2"));
    }
}
