//! `matcha add <sse|stream|buffer>` — appends a capability handler method to
//! the single controller found under `src/controllers`, wiring in the
//! decorator import via `wire::add_import` and splicing the method body via
//! `wire::insert_method`.

use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use crate::wire;

/// Returns `(decorator symbol, method source, dedup marker)` for a known
/// capability, or `None` for an unrecognized one.
///
/// The dedup marker is the route-path string literal baked into that
/// capability's `handler()` template (e.g. `'/events'`, `'/stream'`,
/// `'/data'`). It is unique per capability, unlike the decorator name — both
/// `buffer` and any pre-existing handler can legitimately use `@Get` — so
/// it's what `run` checks for before inserting, to keep `matcha add <cap>`
/// idempotent per capability rather than per decorator.
fn handler(cap: &str) -> Option<(&'static str, String, &'static str)> {
    match cap {
        "sse" => Some((
            "Sse",
            "\n  @Sse('/events')\n  events() {\n    return (async function* () {\n      yield { hello: 'world' };\n    })();\n  }\n".into(),
            "'/events'",
        )),
        "stream" => Some((
            "Stream",
            "\n  @Stream('/stream')\n  stream() {\n    return (async function* () {\n      yield 'chunk';\n    })();\n  }\n".into(),
            "'/stream'",
        )),
        "buffer" => Some((
            "Get",
            "\n  @Get('/data')\n  data() {\n    return Buffer.from('hello');\n  }\n".into(),
            "'/data'",
        )),
        _ => None,
    }
}

/// Finds the single `*.controller.ts` file in `src/controllers`. Errors if
/// there are zero or more than one — this v1 has no way to disambiguate.
fn only_controller() -> std::io::Result<PathBuf> {
    let dir = std::path::Path::new("src/controllers");
    let mut hits: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".controller.ts"))
        .collect();
    match hits.len() {
        1 => Ok(hits.pop().unwrap()),
        0 => Err(Error::new(
            ErrorKind::NotFound,
            "no *.controller.ts in src/controllers",
        )),
        _ => Err(Error::other(
            "multiple controllers; specify which (not yet supported)",
        )),
    }
}

/// Extracts the class name following `export class `. This is a plain text
/// scan rather than a tree-sitter query — `insert_method` does the real
/// grammar-aware work once the name is known.
fn class_name(src: &str) -> Option<String> {
    let idx = src.find("export class ")? + "export class ".len();
    let rest = &src[idx..];
    let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    Some(rest[..end].to_string())
}

pub fn run(cap: &str) -> std::io::Result<()> {
    let (decorator, method, marker) = handler(cap)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, format!("unknown capability {cap}")))?;
    let path = only_controller()?;
    let src = std::fs::read_to_string(&path)?;
    let class = class_name(&src).ok_or_else(|| Error::other("no exported class found"))?;
    if src.contains(marker) {
        println!("→ {cap} already present in {class}");
        return Ok(());
    }
    // wire::add_import merges `decorator` into an existing
    // `import { ... } from '@green-tea/core';` brace when one is already
    // present, rather than appending a second import line for the same
    // module (which would be a duplicate ESM binding — a hard SyntaxError,
    // not a harmless dupe).
    let with_import = wire::add_import(&src, decorator, "@green-tea/core");
    match wire::insert_method(&with_import, &class, &method) {
        Some(out) => {
            std::fs::write(&path, out)?;
            println!("✓ added @{decorator} to {class}");
        }
        None => println!(
            "→ could not edit {}; add @{decorator} handler manually",
            path.display()
        ),
    }
    Ok(())
}
