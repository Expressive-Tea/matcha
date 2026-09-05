//! `matcha add <capability>` — append a ready handler to a controller, with the
//! decorator imports it needs merged into the existing `@green-tea/core` import.
//!
//! Each capability is one row in [`CAPABILITIES`]: what to import, the method to
//! splice, the literal that makes the insert idempotent, and the optional peer
//! dependency the handler needs at runtime. Adding a capability is adding a row.

use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use crate::wire;

/// A handler `matcha add` can splice into a controller.
struct Capability {
    /// Name as typed on the command line.
    name: &'static str,
    /// Symbols merged into the controller's `@green-tea/core` value import.
    imports: &'static [&'static str],
    /// Symbols that must arrive as `import type`, because they appear in a
    /// decorated signature. See [`wire::add_type_import`].
    type_imports: &'static [&'static str],
    /// The method source, spliced before the class's closing brace.
    method: &'static str,
    /// The route-path literal baked into `method`, used to keep the insert
    /// idempotent. It is the marker rather than the decorator name because a
    /// decorator is not unique — `buffer` uses `@Get`, and so does any handler
    /// the user already wrote.
    marker: &'static str,
    /// An optional peer dependency the handler needs at runtime, and the
    /// runtimes that need it. green-tea lazy-loads both, so a missing one is a
    /// clear error at the first request rather than a crash at boot — but it is
    /// still an error, and the person who just generated the handler is the one
    /// who can avoid it.
    peer: Option<(&'static str, &'static str)>,
}

const CAPABILITIES: &[Capability] = &[
    Capability {
        name: "sse",
        // `sse()` and `@header` are what make the stream resumable. Without them
        // an `EventSource` reconnect silently restarts the iterable from its
        // beginning and loses whatever happened in the gap.
        imports: &["Sse", "sse", "header"],
        type_imports: &[],
        method: r#"
  @Sse('/events')
  events(@header('last-event-id') lastEventId?: string) {
    // The browser echoes the last id it saw back on reconnect. green-tea stores
    // nothing — no buffer, no replay — so what the gap means is this handler's
    // to decide: a paged log re-reads from an offset, a live sensor has no past
    // worth delivering.
    let next = lastEventId ? Number(lastEventId) + 1 : 0;
    return (async function* () {
      for (;;) {
        yield sse({ n: next }, { id: String(next) });
        next++;
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
    })();
  }
"#,
        marker: "'/events'",
        peer: None,
    },
    Capability {
        name: "stream",
        imports: &["Stream"],
        type_imports: &[],
        method: r#"
  @Stream('/stream')
  stream() {
    return (async function* () {
      yield 'chunk';
    })();
  }
"#,
        marker: "'/stream'",
        peer: None,
    },
    Capability {
        name: "buffer",
        // "buffer" is the *transport* — the whole response at once, as opposed
        // to the incremental sse/stream/ws ones — and `@Transformer` is what
        // shapes it. The stub used to return `Buffer.from('hello')`, which is
        // not what the name means and never worked: `TransformerFn` returns
        // `body: string`, the default `JsonTransformer` is `JSON.stringify`,
        // and a Node Buffer stringifies to `{"type":"Buffer","data":[...]}`.
        imports: &["Get", "Transformer"],
        type_imports: &[],
        method: r#"
  @Get('/data')
  @Transformer((rows) => ({
    status: 200,
    headers: { 'content-type': 'text/csv' },
    body: (rows as string[]).join('\n'),
  }))
  data() {
    return ['id,name', '1,matcha'];
  }
"#,
        marker: "'/data'",
        peer: None,
    },
    Capability {
        name: "ws",
        imports: &["Ws", "inbound", "channel"],
        type_imports: &[],
        method: r#"
  @Ws('/echo')
  echo(@inbound() incoming: AsyncIterable<string>) {
    // Two iterables: `@inbound()` is what the client sends, and the one returned
    // is what it receives. The pump is deliberately not awaited — the handler
    // returns the outbound channel now and fills it as messages arrive.
    const outbound = channel<string>();
    (async () => {
      for await (const message of incoming) outbound.push(`echo: ${message}`);
      outbound.close();
    })();
    return outbound;
  }
"#,
        marker: "'/echo'",
        peer: Some(("ws", "Node")),
    },
    Capability {
        name: "upload",
        imports: &["Post", "body"],
        type_imports: &["MultipartBody"],
        method: r#"
  @Post('/upload')
  upload(@body() form: MultipartBody) {
    return Object.entries(form.files).map(([field, uploaded]) => {
      // A repeated part name arrives as an array, so a field holds either shape.
      const file = Array.isArray(uploaded) ? uploaded[0] : uploaded;
      return { field, filename: file.filename, size: file.size };
    });
  }
"#,
        marker: "'/upload'",
        peer: Some(("busboy", "every runtime")),
    },
];

/// The capability names, for clap's `value_parser` and for error messages.
pub fn names() -> Vec<&'static str> {
    CAPABILITIES.iter().map(|c| c.name).collect()
}

fn find(name: &str) -> Option<&'static Capability> {
    CAPABILITIES.iter().find(|c| c.name == name)
}

/// Resolves which controller to edit. With `--controller` the path is taken as
/// given; without it there must be exactly one `*.controller.ts` under
/// `src/controllers`, since guessing between several would edit the wrong file
/// as often as the right one.
fn resolve_controller(explicit: Option<&str>) -> std::io::Result<PathBuf> {
    if let Some(path) = explicit {
        let p = PathBuf::from(path);
        return if p.exists() {
            Ok(p)
        } else {
            Err(Error::new(
                ErrorKind::NotFound,
                format!("no such controller: {path}"),
            ))
        };
    }

    let dir = std::path::Path::new("src/controllers");
    let mut hits: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".controller.ts"))
        .collect();
    hits.sort();
    match hits.len() {
        1 => Ok(hits.pop().unwrap()),
        0 => Err(Error::new(
            ErrorKind::NotFound,
            "no *.controller.ts in src/controllers",
        )),
        _ => {
            let list: Vec<String> = hits.iter().map(|p| format!("  {}", p.display())).collect();
            Err(Error::other(format!(
                "{} controllers found — pass --controller <path>:\n{}",
                hits.len(),
                list.join("\n")
            )))
        }
    }
}

/// Extracts the class name following `export class `. A plain text scan rather
/// than a tree-sitter query — `insert_method` does the grammar-aware work once
/// the name is known.
fn class_name(src: &str) -> Option<String> {
    let idx = src.find("export class ")? + "export class ".len();
    let rest = &src[idx..];
    let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    Some(rest[..end].to_string())
}

pub fn run(cap: &str, controller: Option<&str>) -> std::io::Result<()> {
    let capability = find(cap).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            format!("unknown capability {cap} (try: {})", names().join(", ")),
        )
    })?;

    let path = resolve_controller(controller)?;
    let src = std::fs::read_to_string(&path)?;
    let class = class_name(&src).ok_or_else(|| Error::other("no exported class found"))?;
    if src.contains(capability.marker) {
        println!("→ {cap} already present in {class}");
        return Ok(());
    }

    // add_import merges each symbol into an existing
    // `import { ... } from '@green-tea/core';` brace rather than appending a
    // second import line for the same module — which would be a duplicate ESM
    // binding, a hard SyntaxError rather than a harmless dupe.
    let mut edited = src;
    for symbol in capability.imports {
        edited = wire::add_import(&edited, symbol, "@green-tea/core");
    }
    for symbol in capability.type_imports {
        edited = wire::add_type_import(&edited, symbol, "@green-tea/core");
    }

    match wire::insert_method(&edited, &class, capability.method) {
        Some(out) => {
            std::fs::write(&path, out)?;
            println!("✓ added {cap} handler to {class} ({})", path.display());
            if let Some((package, runtimes)) = capability.peer {
                println!(
                    "→ needs the optional peer dependency '{package}' on {runtimes}: npm i {package}"
                );
            }
        }
        None => println!(
            "→ could not edit {}; add the {cap} handler manually",
            path.display()
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_capability_marker_appears_in_its_own_method() {
        for cap in CAPABILITIES {
            assert!(
                cap.method.contains(cap.marker),
                "{}: marker {} is not in the method it guards",
                cap.name,
                cap.marker
            );
        }
    }

    /// The marker is what makes a second `matcha add <cap>` a no-op, so two
    /// capabilities sharing one would make each look already-present after the
    /// other had been added.
    #[test]
    fn markers_are_unique_across_capabilities() {
        for (i, a) in CAPABILITIES.iter().enumerate() {
            for b in &CAPABILITIES[i + 1..] {
                assert_ne!(
                    a.marker, b.marker,
                    "{} and {} share a marker",
                    a.name, b.name
                );
            }
        }
    }

    /// A generated handler that references a symbol it did not import does not
    /// compile, and `matcha add` would have written it anyway.
    #[test]
    fn every_capability_imports_the_symbols_it_uses() {
        for cap in CAPABILITIES {
            for symbol in cap.imports.iter().chain(cap.type_imports) {
                assert!(
                    cap.method.contains(*symbol),
                    "{} imports {symbol} without using it",
                    cap.name
                );
            }
        }
    }

    #[test]
    fn unknown_capability_lists_the_known_ones() {
        let err = run("telepathy", None).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(err.to_string().contains("sse"));
    }
}
