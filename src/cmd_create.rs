//! `matcha create <kind> <Name>` — generate a stub file for `<kind>` and
//! auto-wire it into `src/app.module.ts` via the pure text-editing helpers
//! in `wire`.
//!
//! `module` and `step` are intentionally stubbed as "not implemented yet"
//! for v1 — they will reuse the same `wire` primitives as `controller` in a
//! fast follow.

use std::io::{Error, ErrorKind};
use std::path::Path;

use crate::wire;

fn controller_stub(name: &str) -> String {
    format!(
        "import {{ Route, Get }} from '@green-tea/core';\n\n\
@Route('/{lower}')\nexport class {name}Controller {{\n  \
@Get('/')\n  list() {{\n    return [];\n  }}\n}}\n",
        lower = name.to_lowercase(),
        name = name
    )
}

pub fn run(kind: &str, name: &str, check: bool) -> std::io::Result<()> {
    match kind {
        "controller" => create_controller(name, check),
        "module" | "step" => Err(Error::new(
            ErrorKind::Unsupported,
            format!("create {kind} not implemented yet"),
        )),
        other => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unknown kind {other}"),
        )),
    }
}

fn create_controller(name: &str, check: bool) -> std::io::Result<()> {
    let file = Path::new("src/controllers").join(format!("{}.controller.ts", name.to_lowercase()));
    std::fs::create_dir_all(file.parent().unwrap())?;
    std::fs::write(&file, controller_stub(name))?;
    println!("✓ {}", file.display());

    let symbol = format!("{name}Controller");
    let from = format!("./controllers/{}.controller", name.to_lowercase());
    let module_path = Path::new("src/app.module.ts");
    if !module_path.exists() {
        println!("→ add {symbol} to your @Module manually (no src/app.module.ts found)");
        return Ok(());
    }
    let src = std::fs::read_to_string(module_path)?;
    let imported = wire::add_import(&src, &symbol, &from);
    match wire::add_to_module_array(&imported, "controllers", &symbol) {
        Some(wired) => {
            std::fs::write(module_path, wired)?;
            println!("✓ wired {symbol} into AppModule");
        }
        None => {
            // agreed fallback: emit-only, do NOT corrupt the module
            println!("→ could not auto-wire; add {symbol} to controllers[] in src/app.module.ts");
        }
    }

    if check {
        match crate::check::run(Path::new(".")) {
            Ok(true) => println!("✓ type-check passed"),
            Ok(false) => println!("⚠ type-check reported errors"),
            Err(e) => println!("⚠ could not run type-check: {e}"),
        }
    }

    Ok(())
}
