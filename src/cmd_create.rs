//! `matcha create <kind> <Name>` — generate a stub file for `<kind>` and
//! auto-wire it into `src/app.module.ts` via the pure text-editing helpers
//! in `wire`.

use std::io::{Error, ErrorKind};
use std::path::Path;

use crate::{entry, wire};

pub fn run(kind: &str, name: &str, check: bool) -> std::io::Result<()> {
    match kind {
        "controller" => create_piece(
            name,
            check,
            "controllers",
            "controller",
            "Controller",
            "controllers",
            controller_stub(name),
        ),
        "step" => create_piece(
            name,
            check,
            "steps",
            "step",
            "Step",
            "steps",
            step_stub(name),
        ),
        "provider" => create_piece(
            name,
            check,
            "providers",
            "provider",
            "Provider",
            "providers",
            provider_stub(name),
        ),
        "module" => create_module(name, check),
        other => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unknown kind {other}"),
        )),
    }
}

/// Writes a piece file under `src/<subdir>/` and wires it into the `@Module`
/// in `src/app.module.ts` (`module_key` array). Emit-only + hint on failure.
#[allow(clippy::too_many_arguments)]
fn create_piece(
    name: &str,
    check: bool,
    subdir: &str,
    ext: &str,
    suffix: &str,
    module_key: &str,
    stub: String,
) -> std::io::Result<()> {
    let lower = name.to_lowercase();
    let file = Path::new("src")
        .join(subdir)
        .join(format!("{lower}.{ext}.ts"));
    std::fs::create_dir_all(file.parent().unwrap())?;
    std::fs::write(&file, stub)?;
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

fn maybe_check(check: bool) -> std::io::Result<()> {
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
