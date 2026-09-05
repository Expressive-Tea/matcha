use std::path::Path;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Runtime {
    Node,
    Deno,
    Bun,
    /// Cloudflare Workers. Unlike the other three this is a deploy target as
    /// much as a local one — `wrangler dev` runs it in real workerd, but the
    /// graph is resolved under node, since introspection never touches the
    /// edge handler.
    Edge,
}

impl Runtime {
    pub fn from_str(s: &str) -> Option<Runtime> {
        match s.trim() {
            "node" => Some(Runtime::Node),
            "deno" => Some(Runtime::Deno),
            "bun" => Some(Runtime::Bun),
            "edge" => Some(Runtime::Edge),
            _ => None,
        }
    }
}

fn exists(dir: &Path, name: &str) -> bool {
    dir.join(name).exists()
}

/// Reads `runtime = "..."` from matcha.toml without a TOML dep (one field only).
fn override_from_toml(dir: &Path) -> Option<Runtime> {
    let text = std::fs::read_to_string(dir.join("matcha.toml")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("runtime") {
            let val = rest.trim().trim_start_matches('=').trim().trim_matches('"');
            return Runtime::from_str(val);
        }
    }
    None
}

pub fn detect(dir: &Path) -> Option<Runtime> {
    if let Some(r) = override_from_toml(dir) {
        return Some(r);
    }
    // Before package.json, which an edge project also has.
    if exists(dir, "wrangler.toml") || exists(dir, "wrangler.jsonc") {
        return Some(Runtime::Edge);
    }
    if exists(dir, "deno.json") || exists(dir, "deno.jsonc") {
        return Some(Runtime::Deno);
    }
    if exists(dir, "bun.lockb") || exists(dir, "bun.lock") {
        return Some(Runtime::Bun);
    }
    if exists(dir, "package.json") {
        return Some(Runtime::Node);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(dir: &std::path::Path, name: &str) {
        fs::write(dir.join(name), "{}").unwrap();
    }

    #[test]
    fn deno_wins_over_package_json() {
        let d = tempdir().unwrap();
        touch(d.path(), "package.json");
        touch(d.path(), "deno.json");
        assert_eq!(detect(d.path()), Some(Runtime::Deno));
    }

    #[test]
    fn bun_lock_selects_bun() {
        let d = tempdir().unwrap();
        touch(d.path(), "package.json");
        touch(d.path(), "bun.lockb");
        assert_eq!(detect(d.path()), Some(Runtime::Bun));
    }

    #[test]
    fn package_json_falls_back_to_node() {
        let d = tempdir().unwrap();
        touch(d.path(), "package.json");
        assert_eq!(detect(d.path()), Some(Runtime::Node));
    }

    #[test]
    fn matcha_toml_overrides() {
        let d = tempdir().unwrap();
        touch(d.path(), "deno.json");
        fs::write(d.path().join("matcha.toml"), "runtime = \"bun\"\n").unwrap();
        assert_eq!(detect(d.path()), Some(Runtime::Bun));
    }

    #[test]
    fn wrangler_toml_wins_over_package_json() {
        let d = tempdir().unwrap();
        touch(d.path(), "package.json");
        fs::write(d.path().join("wrangler.toml"), "name = \"x\"\n").unwrap();
        assert_eq!(detect(d.path()), Some(Runtime::Edge));
    }

    #[test]
    fn nothing_detected() {
        let d = tempdir().unwrap();
        assert_eq!(detect(d.path()), None);
    }
}
