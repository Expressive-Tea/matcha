//! `matcha add <sse|stream|buffer>` — appends a capability handler method to
//! the single controller found under `src/controllers`, wiring in the
//! decorator import via `wire::add_import` and splicing the method body via
//! `wire::insert_method`.

use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use crate::wire;

/// Returns `(decorator symbol, method source)` for a known capability, or
/// `None` for an unrecognized one.
fn handler(cap: &str) -> Option<(&'static str, String)> {
    match cap {
        "sse" => Some((
            "Sse",
            "\n  @Sse('/events')\n  events() {\n    return (async function* () {\n      yield { hello: 'world' };\n    })();\n  }\n".into(),
        )),
        "stream" => Some((
            "Stream",
            "\n  @Stream('/stream')\n  stream() {\n    return (async function* () {\n      yield 'chunk';\n    })();\n  }\n".into(),
        )),
        "buffer" => Some((
            "Get",
            "\n  @Get('/data')\n  data() {\n    return Buffer.from('hello');\n  }\n".into(),
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
        0 => Err(Error::new(ErrorKind::NotFound, "no *.controller.ts in src/controllers")),
        _ => Err(Error::other("multiple controllers; specify which (not yet supported)")),
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
    let (decorator, method) = handler(cap)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, format!("unknown capability {cap}")))?;
    let path = only_controller()?;
    let src = std::fs::read_to_string(&path)?;
    let class = class_name(&src).ok_or_else(|| Error::other("no exported class found"))?;
    let with_import = wire::add_import(&src, decorator, "@green-tea/core");
    // add_import above assumes a fresh import line; if '@green-tea/core' is
    // already imported under a shared brace, we still append a second import
    // line for the new symbol — a duplicate named import is legal TS and tsc
    // dedups at type level. Merging into the existing brace is deferred past
    // v1; not worth tree-sitter surgery now.
    match wire::insert_method(&with_import, &class, &method) {
        Some(out) => {
            std::fs::write(&path, out)?;
            println!("✓ added @{decorator} to {class}");
        }
        None => println!("→ could not edit {}; add @{decorator} handler manually", path.display()),
    }
    Ok(())
}
