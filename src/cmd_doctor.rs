//! `matcha doctor` — the checks whose failures are confusing rather than loud.
//!
//! Every check is a file read: no runtime is spawned and no network is touched,
//! so this stays usable on a machine where the project does not yet install.
//! The manifests are scanned rather than parsed, the way `runtime.rs` reads
//! `matcha.toml`, which keeps the CLI free of a JSON dependency for values that
//! are always written one per line by the tools that emit them.

use std::path::Path;

use crate::{entry, runtime, template};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub message: String,
}

fn ok(message: impl Into<String>) -> Finding {
    Finding {
        level: Level::Ok,
        message: message.into(),
    }
}
fn warn(message: impl Into<String>) -> Finding {
    Finding {
        level: Level::Warn,
        message: message.into(),
    }
}
fn fail(message: impl Into<String>) -> Finding {
    Finding {
        level: Level::Fail,
        message: message.into(),
    }
}

pub fn run(dir: &Path) -> std::io::Result<()> {
    let findings = diagnose(dir);
    for finding in &findings {
        let mark = match finding.level {
            Level::Ok => "✓",
            Level::Warn => "⚠",
            Level::Fail => "✗",
        };
        println!("{mark} {}", finding.message);
    }
    if findings.iter().any(|f| f.level == Level::Fail) {
        std::process::exit(1);
    }
    Ok(())
}

/// All checks, in the order a reader wants them: what runtime, what core, can
/// it compile, can it be introspected, will it serve.
pub fn diagnose(dir: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    let rt = runtime::detect(dir);
    out.push(match rt {
        Some(r) => ok(format!("runtime: {r:?}").to_lowercase()),
        None => fail(
            "no runtime detected — expected matcha.toml, deno.json, bun.lock or package.json"
                .to_string(),
        ),
    });
    out.extend(check_core(dir));
    out.extend(check_decorators(dir));
    out.extend(check_entry(dir));
    out.extend(check_peers(dir));
    out
}

/// Reads the value of `"<key>": "<value>"` from a JSON-ish manifest.
///
/// Naive on purpose: it takes the first occurrence and the next quoted string
/// after the colon. Package manifests are machine-written one pair per line, so
/// the cases this would get wrong (the key inside a string, a value spanning
/// lines) do not occur in the files it is pointed at.
fn json_value(src: &str, key: &str) -> Option<String> {
    let at = src.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = &src[at..];
    let colon = rest.find(':')?;
    let after = &rest[colon + 1..];
    let open = after.find('"')?;
    let tail = &after[open + 1..];
    let close = tail.find('"')?;
    Some(tail[..close].to_string())
}

/// The version out of a dependency specifier.
///
/// A plain range (`26.9.0-beta.1`, `^26.7.0`) is already the version. A
/// registry or alias specifier is not: the edge scaffold declares
/// `npm:@jsr/green-tea__core@26.9.0-beta.1` and the deno one
/// `jsr:@green-tea/core@26.9.0-beta.1`, and comparing either of those whole
/// against an installed version reports a mismatch that is not one. The `:`
/// is what tells the two apart — a bare semver range never has one.
fn version_of(spec: &str) -> String {
    if spec.contains(':') {
        spec.rsplit('@').next().unwrap_or(spec).to_string()
    } else {
        spec.to_string()
    }
}

fn read(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name)).ok()
}

/// What the project asks for versus what is on disk, and how both compare to
/// the version this CLI scaffolds against.
///
/// The installed version is read from the package's own `package.json`, never
/// from core's exported `VERSION` constant — that constant is stale in the
/// published artifact (Green-Tea/core#92), so it would report the wrong answer
/// for exactly the check that exists to catch a wrong version.
fn check_core(dir: &Path) -> Vec<Finding> {
    let declared = read(dir, "package.json")
        .and_then(|s| json_value(&s, "@green-tea/core"))
        .or_else(|| read(dir, "deno.json").and_then(|s| json_value(&s, "@green-tea/core")))
        .map(|spec| version_of(&spec));

    let Some(declared) = declared else {
        return vec![fail("no @green-tea/core dependency declared".to_string())];
    };

    let mut out = vec![ok(format!("@green-tea/core declared: {declared}"))];

    // Deno resolves through its own cache rather than node_modules, so absence
    // there is not a finding — only a mismatch is, and only node and bun can
    // show one.
    if let Some(installed) = read(dir, "node_modules/@green-tea/core/package.json")
        .and_then(|s| json_value(&s, "version"))
    {
        if installed != declared.trim_start_matches(['^', '~']) {
            out.push(warn(format!(
                "installed core is {installed}, the manifest asks for {declared} — run your package manager's install"
            )));
        }
    }

    if declared.starts_with('^') || declared.starts_with('~') {
        out.push(warn(format!(
            "core is pinned with a range ({declared}) while it is prerelease-only — \
             npm excludes a prerelease from a range whose comparators carry a different \
             major.minor.patch, so this cannot resolve past its first match. Pin exactly."
        )));
    }

    if declared.trim_start_matches(['^', '~']) != template::CORE_VERSION {
        out.push(warn(format!(
            "this matcha scaffolds against {} — a project on {declared} may not match its generated code",
            template::CORE_VERSION
        )));
    }
    out
}

/// Decorators are the whole surface. Without the compiler flag every `@Route`
/// is a syntax error, and the message points at the decorator rather than at
/// the setting that rejected it.
fn check_decorators(dir: &Path) -> Vec<Finding> {
    let config = ["tsconfig.json", "deno.json", "deno.jsonc"]
        .iter()
        .find_map(|name| read(dir, name).map(|body| (*name, body)));

    match config {
        None => vec![warn("no tsconfig.json or deno.json — cannot confirm decorators are enabled".to_string())],
        Some((name, body)) if body.contains("\"experimentalDecorators\": true") => {
            vec![ok(format!("decorators enabled in {name}"))]
        }
        Some((name, _)) => vec![fail(format!(
            "{name} does not set \"experimentalDecorators\": true — every @Route is a syntax error without it"
        ))],
    }
}

/// `matcha graph` and `matcha explain` import the app rather than serve it, so
/// they need it exported. A project that never exports it works perfectly and
/// simply cannot be introspected, which is worth saying before someone runs
/// `graph` and reads a module-resolution error.
fn check_entry(dir: &Path) -> Vec<Finding> {
    let Some(path) = entry::find(dir) else {
        return vec![warn(format!(
            "no {} — matcha graph and matcha explain need an entry exporting the app",
            entry::candidates()
        ))];
    };
    let name = path
        .strip_prefix(dir)
        .unwrap_or(&path)
        .display()
        .to_string();
    match std::fs::read_to_string(&path) {
        Ok(body) if body.contains("export const app") || body.contains("export { app") => {
            vec![ok(format!("{name} exports the app"))]
        }
        Ok(_) => vec![warn(format!(
            "{name} does not export `app` — matcha graph and matcha explain cannot import it"
        ))],
        Err(e) => vec![warn(format!("could not read {name}: {e}"))],
    }
}

/// Optional peer dependencies, matched to what the source actually uses.
///
/// A grep rather than an analysis: `@Ws(` and `MultipartBody` are the two
/// markers that are unambiguous in practice, and both dependencies are
/// lazy-required by core — so the project compiles either way and fails on the
/// first request that reaches the route. That gap is what this closes.
fn check_peers(dir: &Path) -> Vec<Finding> {
    let sources = collect_sources(&dir.join("src"));
    let manifest = read(dir, "package.json").unwrap_or_default();
    let mut out = Vec::new();

    for (marker, package, why) in [
        ("@Ws(", "ws", "@Ws routes need it on Node"),
        ("MultipartBody", "busboy", "multipart uploads need it"),
    ] {
        if !sources.contains(marker) {
            continue;
        }
        let installed = dir.join("node_modules").join(package).exists();
        if installed || manifest.contains(&format!("\"{package}\"")) {
            out.push(ok(format!("{package} present ({why})")));
        } else {
            out.push(warn(format!(
                "src uses {marker} but '{package}' is not installed — {why}: npm i {package}"
            )));
        }
    }
    out
}

/// Concatenates every `.ts` file under `dir`, recursively. Small trees; the
/// alternative is a walker dependency for a grep.
fn collect_sources(dir: &Path) -> String {
    let mut out = String::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.push_str(&collect_sources(&path));
        } else if path.extension().is_some_and(|e| e == "ts") {
            if let Ok(body) = std::fs::read_to_string(&path) {
                out.push_str(&body);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn levels(findings: &[Finding], needle: &str) -> Vec<Level> {
        findings
            .iter()
            .filter(|f| f.message.contains(needle))
            .map(|f| f.level)
            .collect()
    }

    #[test]
    fn json_value_reads_a_pair() {
        let src = r#"{ "dependencies": { "@green-tea/core": "26.9.0-beta.1" } }"#;
        assert_eq!(
            json_value(src, "@green-tea/core").as_deref(),
            Some("26.9.0-beta.1")
        );
        assert_eq!(json_value(src, "missing"), None);
    }

    #[test]
    fn missing_decorators_is_a_failure_not_a_warning() {
        let d = tempdir().unwrap();
        write(
            d.path(),
            "tsconfig.json",
            r#"{ "compilerOptions": { "strict": true } }"#,
        );
        assert_eq!(
            levels(&check_decorators(d.path()), "experimentalDecorators"),
            vec![Level::Fail]
        );
    }

    #[test]
    fn a_caret_pin_on_a_prerelease_core_is_reported() {
        let d = tempdir().unwrap();
        write(
            d.path(),
            "package.json",
            r#"{ "dependencies": { "@green-tea/core": "^26.7.0-beta.0" } }"#,
        );
        assert_eq!(
            levels(&check_core(d.path()), "pinned with a range"),
            vec![Level::Warn]
        );
    }

    #[test]
    fn an_installed_version_that_differs_from_the_manifest_is_reported() {
        let d = tempdir().unwrap();
        write(
            d.path(),
            "package.json",
            r#"{ "dependencies": { "@green-tea/core": "26.9.0-beta.1" } }"#,
        );
        write(
            d.path(),
            "node_modules/@green-tea/core/package.json",
            r#"{ "version": "26.7.0-beta.0" }"#,
        );
        assert_eq!(
            levels(&check_core(d.path()), "installed core is"),
            vec![Level::Warn]
        );
    }

    #[test]
    fn a_matching_install_reports_nothing_extra() {
        let d = tempdir().unwrap();
        let v = template::CORE_VERSION;
        write(
            d.path(),
            "package.json",
            &format!(r#"{{ "dependencies": {{ "@green-tea/core": "{v}" }} }}"#),
        );
        write(
            d.path(),
            "node_modules/@green-tea/core/package.json",
            &format!(r#"{{ "version": "{v}" }}"#),
        );
        let findings = check_core(d.path());
        assert!(
            findings.iter().all(|f| f.level == Level::Ok),
            "{findings:?}"
        );
    }

    #[test]
    fn version_of_unwraps_registry_and_alias_specifiers() {
        assert_eq!(version_of("26.9.0-beta.1"), "26.9.0-beta.1");
        assert_eq!(version_of("^26.7.0-beta.0"), "^26.7.0-beta.0");
        assert_eq!(
            version_of("jsr:@green-tea/core@26.9.0-beta.1"),
            "26.9.0-beta.1"
        );
        assert_eq!(
            version_of("npm:@jsr/green-tea__core@26.9.0-beta.1"),
            "26.9.0-beta.1"
        );
    }

    /// The edge scaffold installs core through an npm alias, and a doctor that
    /// warned about it would cry wolf on the project it just generated.
    #[test]
    fn the_edge_alias_is_not_reported_as_a_mismatch() {
        let d = tempdir().unwrap();
        let v = template::CORE_VERSION;
        write(
            d.path(),
            "package.json",
            &format!(
                r#"{{ "dependencies": {{ "@green-tea/core": "npm:@jsr/green-tea__core@{v}" }} }}"#
            ),
        );
        write(
            d.path(),
            "node_modules/@green-tea/core/package.json",
            &format!(r#"{{ "name": "@jsr/green-tea__core", "version": "{v}" }}"#),
        );
        let findings = check_core(d.path());
        assert!(
            findings.iter().all(|f| f.level == Level::Ok),
            "{findings:?}"
        );
    }

    #[test]
    fn deno_reads_its_pin_out_of_the_jsr_specifier() {
        let d = tempdir().unwrap();
        let v = template::CORE_VERSION;
        write(
            d.path(),
            "deno.json",
            &format!(r#"{{ "imports": {{ "@green-tea/core": "jsr:@green-tea/core@{v}" }} }}"#),
        );
        assert_eq!(levels(&check_core(d.path()), "declared"), vec![Level::Ok]);
    }

    #[test]
    fn a_ws_route_without_the_ws_package_is_reported() {
        let d = tempdir().unwrap();
        write(
            d.path(),
            "src/controllers/x.controller.ts",
            "@Ws('/echo')\n",
        );
        write(d.path(), "package.json", "{}");
        assert_eq!(
            levels(&check_peers(d.path()), "'ws' is not installed"),
            vec![Level::Warn]
        );
    }

    /// The grep must not fire on a project that never uses the feature.
    #[test]
    fn no_peer_findings_without_the_features_that_need_them() {
        let d = tempdir().unwrap();
        write(d.path(), "src/controllers/x.controller.ts", "@Get('/')\n");
        write(d.path(), "package.json", "{}");
        assert!(check_peers(d.path()).is_empty());
    }

    #[test]
    fn an_entry_that_does_not_export_the_app_is_reported() {
        let d = tempdir().unwrap();
        write(d.path(), "src/app.ts", "const app = createApp({});\n");
        assert_eq!(
            levels(&check_entry(d.path()), "does not export"),
            vec![Level::Warn]
        );
    }
}
