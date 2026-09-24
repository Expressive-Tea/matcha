//! `matcha create <kind> <Name>` — generate a stub file for `<kind>` and
//! auto-wire it into `src/app.module.ts` via the pure text-editing helpers
//! in `wire`.

use std::io::{Error, ErrorKind};
use std::path::Path;

use crate::{entry, wire};

/// A piece that lives inside a module: a file under `src/<subdir>/` plus an
/// entry in one of `@Module`'s arrays.
///
/// `module` is not one of these — it is registered in the `createApp` call
/// rather than in a module array — so it stays a case of its own below.
struct Piece {
    /// Name as typed on the command line.
    kind: &'static str,
    /// Directory under `src/`, and the `@Module` key, which are the same word
    /// for all three.
    subdir: &'static str,
    /// Filename infix: `users.controller.ts`.
    ext: &'static str,
    /// Appended to the given name to form the exported symbol.
    suffix: &'static str,
    stub: fn(&str) -> String,
}

const PIECES: &[Piece] = &[
    Piece {
        kind: "controller",
        subdir: "controllers",
        ext: "controller",
        suffix: "Controller",
        stub: controller_stub,
    },
    Piece {
        kind: "step",
        subdir: "steps",
        ext: "step",
        suffix: "Step",
        stub: step_stub,
    },
    Piece {
        kind: "provider",
        subdir: "providers",
        ext: "provider",
        suffix: "Provider",
        stub: provider_stub,
    },
];

/// Every `create` kind, for clap's `value_parser` and for error messages. One
/// list, so a new kind cannot be accepted by the parser and rejected by `run`
/// (or the reverse).
pub fn kinds() -> Vec<&'static str> {
    ["module"]
        .into_iter()
        .chain(PIECES.iter().map(|p| p.kind))
        .chain(["plugin"])
        .collect()
}

pub fn run(kind: &str, name: &str, check: bool) -> std::io::Result<()> {
    if kind == "module" {
        return create_module(name, check);
    }
    match PIECES.iter().find(|p| p.kind == kind) {
        Some(piece) => create_piece(name, check, piece),
        None => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unknown kind {kind} (try: {})", kinds().join(", ")),
        )),
    }
}

/// Writes a piece file under `src/<subdir>/` and wires it into the `@Module`
/// in `src/app.module.ts`. Emit-only + hint on failure.
fn create_piece(name: &str, check: bool, piece: &Piece) -> std::io::Result<()> {
    let Piece {
        subdir,
        ext,
        suffix,
        ..
    } = *piece;
    let module_key = subdir;
    let lower = name.to_lowercase();
    let file = Path::new("src")
        .join(subdir)
        .join(format!("{lower}.{ext}.ts"));
    std::fs::create_dir_all(file.parent().unwrap())?;
    std::fs::write(&file, (piece.stub)(name))?;
    println!("✓ {}", file.display());

    let symbol = format!("{name}{suffix}");
    let from = format!("./{subdir}/{lower}.{ext}");
    let module_path = Path::new("src/app.module.ts");
    if !module_path.exists() {
        println!("→ add {symbol} to your @Module manually (no src/app.module.ts found)");
        return maybe_check(check);
    }
    let src = std::fs::read_to_string(module_path)?;
    let imported = wire::add_import(&src, &symbol, &from);
    match wire::add_to_module_array(&imported, module_key, &symbol) {
        Some(wired) => {
            std::fs::write(module_path, wired)?;
            println!("✓ wired {symbol} into AppModule");
        }
        None => {
            println!("→ could not auto-wire; add {symbol} to {module_key}[] in src/app.module.ts")
        }
    }
    maybe_check(check)
}

/// Writes `src/<name>.module.ts` and registers it in the `createApp({ modules })`
/// call in the project's entry file. Emit-only + hint on failure.
fn create_module(name: &str, check: bool) -> std::io::Result<()> {
    let lower = name.to_lowercase();
    let file = Path::new("src").join(format!("{lower}.module.ts"));
    std::fs::create_dir_all(file.parent().unwrap())?;
    std::fs::write(&file, module_stub(name))?;
    println!("✓ {}", file.display());

    let symbol = format!("{name}Module");
    let from = format!("./{lower}.module");
    let Some(entry_path) = entry::find(Path::new(".")) else {
        println!(
            "→ register {symbol} in createApp({{ modules }}) manually (no {} found)",
            entry::candidates()
        );
        return maybe_check(check);
    };
    let src = std::fs::read_to_string(&entry_path)?;
    let imported = wire::add_import(&src, &symbol, &from);
    match wire::add_to_createapp_modules(&imported, &symbol) {
        Some(wired) => {
            std::fs::write(&entry_path, wired)?;
            println!("✓ registered {symbol} in createApp");
        }
        None => println!(
            "→ could not auto-register; add {symbol} to createApp modules[] in {}",
            entry_path.display()
        ),
    }
    maybe_check(check)
}

pub(crate) fn maybe_check(check: bool) -> std::io::Result<()> {
    if check {
        match crate::check::run(Path::new(".")) {
            Ok(true) => println!("✓ type-check passed"),
            Ok(false) => println!("⚠ type-check reported errors"),
            Err(e) => println!("⚠ could not run type-check: {e}"),
        }
    }
    Ok(())
}

fn controller_stub(name: &str) -> String {
    let lower = name.to_lowercase();
    format!(
        "import {{ Route, Get }} from '@green-tea/core';\n\n@Route('/{lower}')\nexport class {name}Controller {{\n  @Get('/')\n  list() {{\n    return [];\n  }}\n}}\n"
    )
}

fn step_stub(name: &str) -> String {
    let lower = name.to_lowercase();
    format!(
        "import {{ Step }} from '@green-tea/core';\n\n@Step({{ provides: '{lower}', needs: [] }})\nexport class {name}Step {{\n  run(ctx: Record<string, unknown>) {{\n    return {{ {lower}: null }};\n  }}\n}}\n"
    )
}

fn provider_stub(name: &str) -> String {
    let lower = name.to_lowercase();
    format!(
        "import {{ Provider }} from '@green-tea/core';\n\n@Provider({{ provides: '{lower}' }})\nexport class {name}Provider {{\n  provide() {{\n    return {{ {lower}: null }};\n  }}\n}}\n"
    )
}

fn module_stub(name: &str) -> String {
    let lower = name.to_lowercase();
    format!(
        "import {{ Module }} from '@green-tea/core';\n\n@Module({{ mountpoint: '/{lower}', controllers: [], providers: [], steps: [] }})\nexport class {name}Module {{}}\n"
    )
}
