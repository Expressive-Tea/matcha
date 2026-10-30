//! Introspection queries against a project's app: one query in, one JSON
//! document out.
//!
//! This is the data layer under `matcha graph`, `routes`, `tokens`, `explain`
//! and `openapi`. The CLI renders what comes back, as text or with `--json` as
//! it is; a future `matcha mcp` serves the same documents as tools. Keeping the
//! JSON as the contract is what lets that server be a thin layer over
//! [`query`] rather than a second implementation, and it is why the human
//! formatting lives in Rust (`cmd_graph`) instead of in the generated script.
//!
//! How a query runs: write a short script next to the project, import its entry
//! file, `await app.ready()`, print one JSON envelope, delete the script.
//! `ready()` resolves the graph and is documented as *not* running provider
//! factories, so asking about the graph does not open database connections. It
//! is also required: on a mesh app the graph is not knowable until its teapots
//! have been asked, and `graph()`/`explain()` throw until then.

use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::entry;
use crate::runtime::{self, Runtime};

/// Written into the project root so the runtime resolves `@green-tea/core` from
/// the project's own `node_modules`/import map, then removed. Dotted so it is
/// invisible to a `src/**` glob if a run dies before cleanup.
const SCRIPT_NAME: &str = ".matcha-introspect.ts";

/// What to ask the app. Each variant is one future MCP tool.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    /// The whole graph as data: nodes and routes (`app.graph()`).
    Graph,
    /// The graph as a diagram string; `mermaid` or `dot`.
    Diagram(Diagram),
    /// Every route with its transport and the chain it runs.
    Routes,
    /// Every token: who provides it and who needs it.
    Tokens,
    /// One route's chain in execution order, by declared pattern.
    Explain(String),
    /// The structural OpenAPI 3.1 document.
    OpenApi {
        title: Option<String>,
        version: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Diagram {
    Mermaid,
    Dot,
}

/// A query the app answered with a refusal rather than data — a route pattern
/// that is not registered, say. Carried as JSON so a caller (a person, an agent,
/// an MCP client) gets the same machine-readable reason.
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal(pub Value);

impl Refusal {
    pub fn message(&self) -> &str {
        self.0["message"].as_str().unwrap_or("query refused")
    }
}

/// Where the app lives: the project directory and, optionally, an entry file
/// other than the default `src/app.ts` / `src/main.ts`.
pub struct Target<'a> {
    pub dir: &'a Path,
    pub entry: Option<&'a str>,
}

/// Runs `query` against the app in `target`. `Ok(Ok(data))` is the answer,
/// `Ok(Err(refusal))` is the app saying no in a structured way, and `Err` is
/// everything that stopped the question being asked — no entry, no runtime, a
/// type error or a failed boot. The child's stderr is inherited, so those reach
/// the terminal as they happen.
pub fn query(target: &Target, query: &Query) -> std::io::Result<Result<Value, Refusal>> {
    let entry_path = resolve_entry(target)?;
    let rt = runtime::detect(target.dir).ok_or_else(|| {
        Error::new(
            ErrorKind::NotFound,
            "no runtime detected (need deno.json, bun.lock, or package.json)",
        )
    })?;

    let script_path = target.dir.join(SCRIPT_NAME);
    std::fs::write(&script_path, script(&entry_path, &body(query)))?;
    let result = run(rt, target.dir, SCRIPT_NAME);
    std::fs::remove_file(&script_path).ok();

    let output = result?;
    if !output.status.success() {
        return Err(Error::other(format!(
            "the app could not be inspected (exit {})",
            output.status.code().unwrap_or(1)
        )));
    }
    parse_envelope(&String::from_utf8_lossy(&output.stdout))
}

/// The envelope is the script's last stdout line, so anything the app logs
/// while booting cannot be mistaken for the answer.
fn parse_envelope(stdout: &str) -> std::io::Result<Result<Value, Refusal>> {
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let envelope: Value = serde_json::from_str(line).map_err(|e| {
        Error::new(
            ErrorKind::InvalidData,
            format!("the introspection script printed no answer ({e})"),
        )
    })?;
    match envelope["ok"].as_bool() {
        Some(true) => Ok(Ok(envelope["data"].clone())),
        Some(false) => Ok(Err(Refusal(envelope["error"].clone()))),
        None => Err(Error::new(
            ErrorKind::InvalidData,
            "malformed introspection answer",
        )),
    }
}

/// The JavaScript expression (or statements ending in `data = …`) that computes
/// the answer. Everything is plain data built from public `App` methods.
fn body(query: &Query) -> String {
    match query {
        Query::Graph => "data = app.graph();".into(),
        Query::Diagram(Diagram::Mermaid) => "data = app.toMermaid();".into(),
        Query::Diagram(Diagram::Dot) => "data = app.toDOT();".into(),
        Query::Routes => "data = app.graph().routes;".into(),
        Query::Tokens => TOKENS.into(),
        Query::Explain(pattern) => format!(
            r#"try {{
  data = app.explain({pattern});
}} catch {{
  refuse({{
    code: 'route_not_found',
    message: `no route matching pattern ${{{pattern}}}`,
    registered: app.graph().routes.map((r) => ({{ method: r.method, pattern: r.pattern }})),
  }});
}}"#,
            pattern = js_string(pattern)
        ),
        Query::OpenApi { title, version } => {
            let mut fields = Vec::new();
            if let Some(t) = title {
                fields.push(format!("title: {}", js_string(t)));
            }
            if let Some(v) = version {
                fields.push(format!("version: {}", js_string(v)));
            }
            // No info object at all lets core apply its own defaults rather than
            // having the CLI invent a title.
            let info = if fields.is_empty() {
                String::new()
            } else {
                format!("{{ {} }}", fields.join(", "))
            };
            format!("data = app.openapi({info});")
        }
    }
}

/// Tokens, from the graph's nodes plus each route's handler (only `explain`
/// knows what a handler needs). Sorted, so the answer is stable across runs.
const TOKENS: &str = r#"const tokens = new Map();
const entry = (t) => tokens.get(t) ?? tokens.set(t, { token: t, providedBy: [], neededBy: [] }).get(t);
const graph = app.graph();
for (const n of graph.nodes) {
  for (const t of n.provides) entry(t).providedBy.push({ kind: n.kind, name: n.name, origin: n.origin });
  for (const t of n.needs) entry(t).neededBy.push({ kind: n.kind, name: n.name, origin: n.origin });
}
for (const pattern of new Set(graph.routes.map((r) => r.pattern))) {
  for (const n of app.explain(pattern).chain) {
    if (n.kind !== 'handler') continue;
    for (const t of n.needs) entry(t).neededBy.push({ kind: 'handler', name: n.name, origin: n.origin, route: pattern });
  }
}
data = [...tokens.values()].sort((a, b) => a.token.localeCompare(b.token));"#;

fn resolve_entry(target: &Target) -> std::io::Result<PathBuf> {
    match target.entry {
        Some(explicit) => {
            if !target.dir.join(explicit).exists() {
                return Err(Error::new(ErrorKind::NotFound, format!("no such entry: {explicit}")));
            }
            Ok(PathBuf::from(explicit))
        }
        None => entry::find(target.dir)
            .map(|p| p.strip_prefix(target.dir).unwrap_or(&p).to_path_buf())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!(
                        "no {} found — pass --entry <file>. It must export the app: `export const app = createApp(...)`",
                        entry::candidates()
                    ),
                )
            }),
    }
}

/// The generated script: import the app, resolve the graph, run `body`, print
/// one envelope. `refuse` ends the script with a structured error instead of
/// data.
fn script(entry_path: &Path, body: &str) -> String {
    format!(
        "// generated by matcha; safe to delete\n\
         import {{ app }} from {};\n\n\
         if (!app) {{\n  console.error('{} does not export `app` — matcha needs `export const app = createApp(...)`');\n  process.exit(1);\n}}\n\
         const refuse = (error) => {{\n  console.log(JSON.stringify({{ ok: false, error }}));\n  process.exit(0);\n}};\n\
         await app.ready();\n\
         let data;\n\
         {body}\n\
         console.log(JSON.stringify({{ ok: true, data }}));\n",
        js_string(&specifier(entry_path)),
        entry_path.display(),
    )
}

/// Runs `script` under `rt` in `dir`, stdout captured, stderr inherited.
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
pub(crate) fn js_string(value: &str) -> String {
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
    fn script_imports_the_entry_awaits_ready_and_prints_one_envelope() {
        let out = script(Path::new("src/app.ts"), "data = 1;");
        assert!(out.contains("import { app } from './src/app.ts';"));
        assert!(out.contains("await app.ready();"));
        assert!(out.contains("data = 1;"));
        assert!(out.contains("JSON.stringify({ ok: true, data })"));
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
    fn the_envelope_is_the_last_line_so_boot_logs_cannot_pose_as_the_answer() {
        let out = "{\"level\":\"info\",\"msg\":\"booted\"}\n{\"ok\":true,\"data\":[1]}\n";
        assert_eq!(parse_envelope(out).unwrap(), Ok(serde_json::json!([1])));
    }

    #[test]
    fn a_refusal_comes_back_structured() {
        let out =
            "{\"ok\":false,\"error\":{\"code\":\"route_not_found\",\"message\":\"no route\"}}";
        let refusal = parse_envelope(out).unwrap().unwrap_err();
        assert_eq!(refusal.0["code"], "route_not_found");
        assert_eq!(refusal.message(), "no route");
    }

    #[test]
    fn explain_escapes_the_pattern() {
        assert!(body(&Query::Explain("/a'b".into())).contains(r"app.explain('/a\'b')"));
    }
}
