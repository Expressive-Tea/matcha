//! `matcha graph` and `matcha explain <pattern>` — introspection of the
//! dependency graph the framework builds from `needs`/`provides`.
//!
//! Both work the same way: write a short script next to the project, import its
//! entry file, `await app.ready()`, print, delete the script. `ready()` is the
//! reason this is cheap and safe to run — it resolves the graph and is
//! documented as deliberately *not* running provider factories, so drawing a
//! diagram does not open your database connections. It is also required rather
//! than optional: on a mesh app the graph is not knowable until its teapots
//! have been asked, and `graph()`/`explain()` throw until then.
//!
//! The formatting lives in the generated script rather than here on purpose.
//! Rust would have to parse the JSON back to lay out a chain, and nothing about
//! that layout needs Rust — `console.log` already has the values.

use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::entry;
use crate::runtime::{self, Runtime};

/// Written into the project root so the runtime resolves `@green-tea/core` from
/// the project's own `node_modules`/import map, then removed. Dotted so it is
/// invisible to a `src/**` glob if a run dies before cleanup.
const SCRIPT_NAME: &str = ".matcha-introspect.ts";

/// `matcha graph` — the whole graph, as Mermaid, Graphviz DOT, or JSON.
pub fn graph(
    dir: &Path,
    format: &str,
    entry_override: Option<&str>,
    out: Option<&str>,
) -> std::io::Result<()> {
    let print = match format {
        "mermaid" => "console.log(app.toMermaid());",
        "dot" => "console.log(app.toDOT());",
        "json" => "console.log(JSON.stringify(app.graph(), null, 2));",
        other => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("unknown format '{other}' (mermaid, dot, json)"),
            ))
        }
    };
    emit(dir, entry_override, out, print)
}

/// `matcha explain <pattern>` — one route's chain, in execution order, with
/// each node's needs and provides.
///
/// The argument is the route *pattern* as declared (`/users/:id`), not a
/// concrete URL: `explainRoute` matches the pattern string exactly. A miss
/// therefore lists what is actually registered, which is the only useful thing
/// to say to someone who just guessed wrong.
pub fn explain(
    dir: &Path,
    route: &str,
    entry_override: Option<&str>,
    out: Option<&str>,
) -> std::io::Result<()> {
    let route = js_string(route);
    let body = format!(
        r#"let explained;
try {{
  explained = app.explain({route});
}} catch {{
  const patterns = app.graph().routes.map((r) => `  ${{r.method.padEnd(6)}} ${{r.pattern}}`);
  console.error(`no route matching pattern {route}\n\nregistered patterns:\n${{patterns.join('\n')}}`);
  process.exit(1);
}}
const lines = [`${{explained.method}} ${{explained.pattern}}  [${{explained.transport}}]`];
const width = Math.max(...explained.chain.map((n) => n.kind.length));
for (const node of explained.chain) {{
  const needs = node.needs.length ? `  needs: ${{node.needs.join(', ')}}` : '';
  const provides = node.provides.length ? `  provides: ${{node.provides.join(', ')}}` : '';
  lines.push(`  ${{node.kind.padEnd(width)}}  ${{node.name}}${{needs}}${{provides}}  (${{node.origin}})`);
}}
console.log(lines.join('\n'));"#
    );
    emit(dir, entry_override, out, &body)
}

/// `matcha openapi` — the structural OpenAPI 3.1 document for the route table.
///
/// Structural is the honest word for it: green-tea derives paths, methods and
/// parameters from the decorators, which is everything the graph knows. It does
/// not invent response schemas, so what comes out describes the surface rather
/// than the payloads.
pub fn openapi(
    dir: &Path,
    title: Option<&str>,
    version: Option<&str>,
    entry_override: Option<&str>,
    out: Option<&str>,
) -> std::io::Result<()> {
    // `app.openapi()` takes an optional info object; passing nothing lets core
    // apply its own defaults rather than having the CLI invent a title.
    let info = match (title, version) {
        (None, None) => String::new(),
        (t, v) => {
            let mut fields = Vec::new();
            if let Some(t) = t {
                fields.push(format!("title: {}", js_string(t)));
            }
            if let Some(v) = v {
                fields.push(format!("version: {}", js_string(v)));
            }
            format!("{{ {} }}", fields.join(", "))
        }
    };
    emit(
        dir,
        entry_override,
        out,
        &format!("console.log(JSON.stringify(app.openapi({info}), null, 2));"),
    )
}

/// Resolves the entry, generates the script around `body`, runs it under the
/// detected runtime, and routes its stdout to a file or to ours.
fn emit(
    dir: &Path,
    entry_override: Option<&str>,
    out: Option<&str>,
    body: &str,
) -> std::io::Result<()> {
    let entry_path = match entry_override {
        Some(explicit) => {
            let p = dir.join(explicit);
            if !p.exists() {
                return Err(Error::new(ErrorKind::NotFound, format!("no such entry: {explicit}")));
            }
            PathBuf::from(explicit)
        }
        None => entry::find(dir).map(|p| p.strip_prefix(dir).unwrap_or(&p).to_path_buf()).ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                format!(
                    "no {} found — pass --entry <file>. It must export the app: `export const app = createApp(...)`",
                    entry::candidates()
                ),
            )
        })?,
    };

    let rt = runtime::detect(dir).ok_or_else(|| {
        Error::new(
            ErrorKind::NotFound,
            "no runtime detected (need deno.json, bun.lock, or package.json)",
        )
    })?;

    let script_path = dir.join(SCRIPT_NAME);
    std::fs::write(&script_path, script(&entry_path, body))?;
    let result = run(rt, dir, SCRIPT_NAME);
    std::fs::remove_file(&script_path).ok();

    let output = result?;
    // The child has already written its own diagnosis to the inherited stderr —
    // a type error, a boot failure, or the script's own "no route matching
    // pattern" with the registered ones listed. Wrapping that in a second
    // `error: npx exited with 1` adds a line and no information, so the exit
    // code is relayed and nothing is said on top of it.
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    match out {
        Some(file) => {
            std::fs::write(dir.join(file), &stdout)?;
            println!("✓ {file}");
        }
        None => print!("{stdout}"),
    }
    Ok(())
}

/// The generated script: import the app, resolve the graph, run `body`.
fn script(entry_path: &Path, body: &str) -> String {
    format!(
        "// generated by matcha; safe to delete\nimport {{ app }} from {};\n\nif (!app) {{\n  console.error('{} does not export `app` — matcha needs `export const app = createApp(...)`');\n  process.exit(1);\n}}\nawait app.ready();\n{body}\n",
        js_string(&specifier(entry_path)),
        entry_path.display(),
    )
}

/// Runs `script` under `rt` in `dir`. The child's stderr is inherited rather
/// than captured: a type error or a boot failure is the reason someone runs
/// this, and it should reach them as it happens.
fn run(rt: Runtime, dir: &Path, script: &str) -> std::io::Result<std::process::Output> {
    let (prog, args) = runner(rt, script);
    let child = Command::new(prog)
        .args(&args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| Error::new(e.kind(), format!("could not run {prog}: {e}")))?;
    child.wait_with_output()
}

fn runner(rt: Runtime, script: &str) -> (&'static str, Vec<String>) {
    let s = script.to_string();
    match rt {
        // Edge resolves its graph under node: `src/app.ts` imports core, never
        // `edgeHandler`, so nothing here needs workerd — and workerd could not
        // print to a terminal anyway.
        Runtime::Node | Runtime::Edge => ("npx", vec!["tsx".into(), s]),
        // `ready()` opens no provider connections, but a mesh app does reach its
        // teapots there, so net is part of resolving the graph rather than extra.
        Runtime::Deno => (
            "deno",
            vec![
                "run".into(),
                "--allow-read".into(),
                "--allow-env".into(),
                "--allow-net".into(),
                s,
            ],
        ),
        Runtime::Bun => ("bun", vec!["run".into(), s]),
    }
}

/// A project-root-relative path as an ES module specifier: forward slashes,
/// always `./`-prefixed, extension kept so Deno resolves it without leaning on
/// `sloppy-imports`.
fn specifier(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    format!("./{}", raw.strip_prefix("./").unwrap_or(&raw))
}

/// Single-quoted JS string literal. Route patterns reach this from argv, so the
/// quote and the backslash have to be escaped rather than assumed absent.
fn js_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specifier_is_dot_slash_prefixed_with_extension() {
        assert_eq!(specifier(Path::new("src/app.ts")), "./src/app.ts");
        assert_eq!(specifier(Path::new("./src/app.ts")), "./src/app.ts");
    }

    #[test]
    fn js_string_escapes_quote_and_backslash() {
        assert_eq!(js_string("a'b"), r"'a\'b'");
        assert_eq!(js_string(r"a\b"), r"'a\\b'");
    }

    #[test]
    fn script_imports_the_entry_and_awaits_ready() {
        let out = script(Path::new("src/app.ts"), "console.log(1);");
        assert!(out.contains("import { app } from './src/app.ts';"));
        assert!(out.contains("await app.ready();"));
        assert!(out.contains("console.log(1);"));
    }

    #[test]
    fn node_runs_through_tsx() {
        assert_eq!(
            runner(Runtime::Node, ".matcha-introspect.ts"),
            (
                "npx",
                vec!["tsx".to_string(), ".matcha-introspect.ts".to_string()]
            )
        );
    }

    #[test]
    fn unknown_format_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let err = graph(d.path(), "svg", None, None).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
}
