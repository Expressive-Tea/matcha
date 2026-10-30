//! `matcha graph`, `routes`, `tokens`, `explain` and `openapi`: the CLI over
//! [`crate::introspect`].
//!
//! Each command asks one [`Query`] and renders the JSON that comes back, as
//! text for a person or, with `--json`, unchanged for a program. An agent
//! inspecting an app uses the second form; the future `matcha mcp` serves the
//! same documents without going through this file at all.

use std::io::{Error, ErrorKind};
use std::path::Path;

use serde_json::Value;

use crate::introspect::{self, Diagram, Query, Refusal, Target};

/// How a command prints its answer.
#[derive(Clone, Copy, PartialEq)]
pub enum Render {
    Text,
    Json,
}

/// `matcha graph` — the whole graph, as Mermaid, Graphviz DOT, or JSON.
pub fn graph(
    dir: &Path,
    format: &str,
    entry: Option<&str>,
    out: Option<&str>,
) -> std::io::Result<()> {
    let (query, render) = match format {
        "mermaid" => (Query::Diagram(Diagram::Mermaid), Render::Text),
        "dot" => (Query::Diagram(Diagram::Dot), Render::Text),
        "json" => (Query::Graph, Render::Json),
        other => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("unknown format '{other}' (mermaid, dot, json)"),
            ))
        }
    };
    answer(dir, entry, out, &query, render, |data| {
        data.as_str().unwrap_or_default().to_string()
    })
}

/// `matcha routes` — every route, its transport, and the chain it runs.
pub fn routes(
    dir: &Path,
    entry: Option<&str>,
    out: Option<&str>,
    render: Render,
) -> std::io::Result<()> {
    answer(dir, entry, out, &Query::Routes, render, routes_text)
}

/// `matcha tokens` — every token, who provides it and who needs it. The
/// question to ask before writing `@needs('x')`: does anything provide `x`?
pub fn tokens(
    dir: &Path,
    entry: Option<&str>,
    out: Option<&str>,
    render: Render,
) -> std::io::Result<()> {
    answer(dir, entry, out, &Query::Tokens, render, tokens_text)
}

/// `matcha explain <pattern>` — one route's chain, in execution order, with
/// each node's needs and provides.
///
/// The argument is the route *pattern* as declared (`/users/:id`), not a
/// concrete URL: `explain` matches the pattern string exactly. A miss lists
/// what is actually registered, which is the only useful thing to say to
/// someone who just guessed wrong.
pub fn explain(
    dir: &Path,
    route: &str,
    entry: Option<&str>,
    out: Option<&str>,
    render: Render,
) -> std::io::Result<()> {
    answer(
        dir,
        entry,
        out,
        &Query::Explain(route.to_string()),
        render,
        explain_text,
    )
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
    entry: Option<&str>,
    out: Option<&str>,
) -> std::io::Result<()> {
    let query = Query::OpenApi {
        title: title.map(str::to_string),
        version: version.map(str::to_string),
    };
    answer(dir, entry, out, &query, Render::Json, |_| String::new())
}

/// Asks, renders, and writes to `out` or stdout. A refusal goes to stderr — as
/// JSON under `--json`, so a program reads the same shape either way — and
/// exits 1. A failed run has already printed its own diagnosis (the child's
/// stderr is inherited), so it exits with no second message on top.
fn answer(
    dir: &Path,
    entry: Option<&str>,
    out: Option<&str>,
    query: &Query,
    render: Render,
    text: impl Fn(&Value) -> String,
) -> std::io::Result<()> {
    let target = Target { dir, entry };
    let data = match introspect::query(&target, query) {
        Ok(Ok(data)) => data,
        Ok(Err(refusal)) => {
            report_refusal(&refusal, render);
            std::process::exit(1);
        }
        Err(e) if e.kind() == ErrorKind::Other => std::process::exit(1),
        Err(e) => return Err(e),
    };

    let mut printed = match render {
        Render::Json => serde_json::to_string_pretty(&data).map_err(Error::other)?,
        Render::Text => text(&data),
    };
    if !printed.ends_with('\n') {
        printed.push('\n');
    }
    match out {
        Some(file) => {
            std::fs::write(dir.join(file), &printed)?;
            println!("✓ {file}");
        }
        None => print!("{printed}"),
    }
    Ok(())
}

fn report_refusal(refusal: &Refusal, render: Render) {
    if render == Render::Json {
        eprintln!("{}", serde_json::json!({ "error": refusal.0 }));
        return;
    }
    eprintln!("{}", refusal.message());
    if let Some(registered) = refusal.0["registered"].as_array() {
        eprintln!("\nregistered patterns:");
        for r in registered {
            eprintln!("  {:<6} {}", str_of(&r["method"]), str_of(&r["pattern"]));
        }
    }
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn list_of(v: &Value) -> Vec<&str> {
    v.as_array()
        .map(|a| a.iter().map(str_of).collect())
        .unwrap_or_default()
}

fn routes_text(data: &Value) -> String {
    let routes = data.as_array().cloned().unwrap_or_default();
    let width = routes
        .iter()
        .map(|r| str_of(&r["method"]).len())
        .max()
        .unwrap_or(0);
    routes
        .iter()
        .map(|r| {
            let chain = list_of(&r["chain"]).join(" → ");
            format!(
                "{:<width$}  {}  [{}]  {}",
                str_of(&r["method"]),
                str_of(&r["pattern"]),
                str_of(&r["transport"]),
                chain
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tokens_text(data: &Value) -> String {
    let names = |v: &Value| {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|n| match n["route"].as_str() {
                        Some(route) => format!("{} ({route})", str_of(&n["name"])),
                        None => str_of(&n["name"]).to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    };
    data.as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|t| {
            let provided = names(&t["providedBy"]);
            let needed = names(&t["neededBy"]);
            format!(
                "{}\n  provided by: {}\n  needed by:   {}",
                str_of(&t["token"]),
                if provided.is_empty() {
                    if t["source"] == "request" {
                        "the request (built in)"
                    } else {
                        "— (nothing provides it)"
                    }
                } else {
                    &provided
                },
                if needed.is_empty() { "—" } else { &needed },
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn explain_text(data: &Value) -> String {
    let chain = data["chain"].as_array().cloned().unwrap_or_default();
    let width = chain
        .iter()
        .map(|n| str_of(&n["kind"]).len())
        .max()
        .unwrap_or(0);
    let mut lines = vec![format!(
        "{} {}  [{}]",
        str_of(&data["method"]),
        str_of(&data["pattern"]),
        str_of(&data["transport"])
    )];
    for n in &chain {
        let needs = list_of(&n["needs"]);
        let provides = list_of(&n["provides"]);
        let mut line = format!("  {:<width$}  {}", str_of(&n["kind"]), str_of(&n["name"]));
        if !needs.is_empty() {
            line.push_str(&format!("  needs: {}", needs.join(", ")));
        }
        if !provides.is_empty() {
            line.push_str(&format!("  provides: {}", provides.join(", ")));
        }
        line.push_str(&format!("  ({})", str_of(&n["origin"])));
        lines.push(line);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_format_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let err = graph(d.path(), "svg", None, None).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn explain_text_keeps_the_layout_it_had() {
        let data = json!({
            "method": "GET", "pattern": "/users/:id", "transport": "http",
            "chain": [
                { "kind": "provider", "name": "db", "needs": [], "provides": ["db"], "origin": "src/db.ts" },
                { "kind": "handler", "name": "Users.get", "needs": ["db"], "provides": [], "origin": "src/users.ts" }
            ]
        });
        assert_eq!(
            explain_text(&data),
            "GET /users/:id  [http]\n  provider  db  provides: db  (src/db.ts)\n  handler   Users.get  needs: db  (src/users.ts)"
        );
    }

    #[test]
    fn tokens_text_names_the_request_for_its_built_in_tokens() {
        let data = json!([{ "token": "req", "source": "request", "providedBy": [], "neededBy": [{ "kind": "step", "name": "user" }] }]);
        assert_eq!(
            tokens_text(&data),
            "req\n  provided by: the request (built in)\n  needed by:   user"
        );
    }

    #[test]
    fn tokens_text_says_when_nothing_provides_a_token() {
        let data = json!([{ "token": "user", "providedBy": [], "neededBy": [{ "kind": "handler", "name": "Me.get", "route": "/me" }] }]);
        assert_eq!(
            tokens_text(&data),
            "user\n  provided by: — (nothing provides it)\n  needed by:   Me.get (/me)"
        );
    }
}
