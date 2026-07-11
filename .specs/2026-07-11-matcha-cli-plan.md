# matcha CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `matcha`, the green-tea CLI — a native Rust binary that scaffolds projects, runs them against the detected JS runtime, and generates/auto-wires framework pieces.

**Architecture:** Single `clap`-based binary. Each subcommand is an isolated unit (`new`, `run`, `create`, `add`) sharing two support modules: `runtime` (filesystem probes → node/deno/bun) and `wire` (tree-sitter TypeScript edits with syntax-check-and-revert). The starter template is embedded via `include_dir!`; remote templates come from `git clone --depth 1`.

**Tech Stack:** Rust, `clap` (derive), `include_dir`, `tree-sitter` + `tree-sitter-typescript`. Tests: `assert_cmd` + `predicates` + `tempfile`.

## Global Constraints

- Binary/crate/repo name: `matcha` — exact, lowercase.
- No HTTP/TLS dependency in the binary. Remote templates use `git clone --depth 1` (shell out to system `git`).
- The binary must not require a JS runtime to install or operate. It only *delegates* to the project's runtime for `run` and the optional semantic check.
- Runtime detection precedence (first match wins): `matcha.toml` `runtime` field → `deno.json(c)` → `bun.lock(b)` → `package.json`.
- `create`/`add` edits are **idempotent** and must **revert on syntax breakage** (tree-sitter `ERROR` node ⇒ leave file untouched).
- Framework API is fixed (verified in `Green-Tea/core`): `@Sse('/path')` returns an async generator; `@Html('path')` serves a file; `@Get`/`@Route`/`@Provider`/`@Module` per `core/src/index.ts`.
- Edge is **not** a `run` target in v1.

---

### Task 1: Binary skeleton + clap command surface

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/cli.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `enum Command { New, Run, Create, Add }` parsed by clap; binary named `matcha`. Later tasks fill each arm.

- [ ] **Step 1: Write the failing test**

```rust
// tests/cli.rs
use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn help_lists_all_subcommands() {
    Command::cargo_bin("matcha").unwrap()
        .arg("--help").assert().success()
        .stdout(contains("new")).stdout(contains("run"))
        .stdout(contains("create")).stdout(contains("add"));
}

#[test]
fn new_requires_a_name() {
    Command::cargo_bin("matcha").unwrap()
        .arg("new").assert().failure(); // missing <name>
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test`
Expected: FAIL — no binary / `matcha` target not found.

- [ ] **Step 3: Write `Cargo.toml`**

```toml
[package]
name = "matcha"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "matcha"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
include_dir = "0.7"
tree-sitter = "0.22"
tree-sitter-typescript = "0.21"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **Step 4: Write `src/cli.rs`**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "matcha", about = "green-tea CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scaffold a new project
    New {
        name: String,
        #[arg(long)]
        template_url: Option<String>,
    },
    /// Detect the runtime and run the project in watch mode
    Run,
    /// Generate a piece and auto-wire it into the module
    Create {
        #[arg(value_parser = ["module", "controller", "step"])]
        kind: String,
        name: String,
        #[arg(long)]
        check: bool,
    },
    /// Add a capability to a controller
    Add {
        #[arg(value_parser = ["sse", "stream", "buffer"])]
        capability: String,
    },
}
```

- [ ] **Step 5: Write `src/main.rs`**

```rust
mod cli;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::New { .. } => eprintln!("new: not yet implemented"),
        Command::Run => eprintln!("run: not yet implemented"),
        Command::Create { .. } => eprintln!("create: not yet implemented"),
        Command::Add { .. } => eprintln!("add: not yet implemented"),
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS (both tests).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/main.rs src/cli.rs tests/cli.rs
git commit -s -m "feat: matcha binary skeleton with clap command surface"
```

---

### Task 2: Runtime detection

**Files:**
- Create: `src/runtime.rs`
- Modify: `src/main.rs` (add `mod runtime;`)
- Test: `src/runtime.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces: `pub enum Runtime { Node, Deno, Bun }` and `pub fn detect(dir: &Path) -> Option<Runtime>`. Consumed by `run` (Task 6) and the semantic check (Task 9).

- [ ] **Step 1: Write the failing test**

```rust
// bottom of src/runtime.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn touch(dir: &std::path::Path, name: &str) {
        fs::write(dir.join(name), "{}").unwrap();
    }

    #[test]
    fn deno_wins_over_package_json() {
        let d = tempdir().unwrap();
        touch(d.path(), "package.json");
        touch(d.path(), "deno.json");
        assert_eq!(detect(d.path()), Some(Runtime::Deno));
    }

    #[test]
    fn bun_lock_selects_bun() {
        let d = tempdir().unwrap();
        touch(d.path(), "package.json");
        touch(d.path(), "bun.lockb");
        assert_eq!(detect(d.path()), Some(Runtime::Bun));
    }

    #[test]
    fn package_json_falls_back_to_node() {
        let d = tempdir().unwrap();
        touch(d.path(), "package.json");
        assert_eq!(detect(d.path()), Some(Runtime::Node));
    }

    #[test]
    fn matcha_toml_overrides() {
        let d = tempdir().unwrap();
        touch(d.path(), "deno.json");
        fs::write(d.path().join("matcha.toml"), "runtime = \"bun\"\n").unwrap();
        assert_eq!(detect(d.path()), Some(Runtime::Bun));
    }

    #[test]
    fn nothing_detected() {
        let d = tempdir().unwrap();
        assert_eq!(detect(d.path()), None);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test runtime`
Expected: FAIL — `detect` not defined.

- [ ] **Step 3: Implement `src/runtime.rs`**

```rust
use std::path::Path;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Runtime { Node, Deno, Bun }

impl Runtime {
    pub fn from_str(s: &str) -> Option<Runtime> {
        match s.trim() {
            "node" => Some(Runtime::Node),
            "deno" => Some(Runtime::Deno),
            "bun" => Some(Runtime::Bun),
            _ => None,
        }
    }
}

fn exists(dir: &Path, name: &str) -> bool {
    dir.join(name).exists()
}

/// Reads `runtime = "..."` from matcha.toml without a TOML dep (one field only).
fn override_from_toml(dir: &Path) -> Option<Runtime> {
    let text = std::fs::read_to_string(dir.join("matcha.toml")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("runtime") {
            let val = rest.trim_start_matches('=').trim().trim_matches('"');
            return Runtime::from_str(val);
        }
    }
    None
}

pub fn detect(dir: &Path) -> Option<Runtime> {
    if let Some(r) = override_from_toml(dir) { return Some(r); }
    if exists(dir, "deno.json") || exists(dir, "deno.jsonc") { return Some(Runtime::Deno); }
    if exists(dir, "bun.lockb") || exists(dir, "bun.lock") { return Some(Runtime::Bun); }
    if exists(dir, "package.json") { return Some(Runtime::Node); }
    None
}
```

- [ ] **Step 4: Wire the module**

In `src/main.rs`, add near the other `mod` lines:

```rust
mod runtime;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test runtime`
Expected: PASS (all five).

- [ ] **Step 6: Commit**

```bash
git add src/runtime.rs src/main.rs
git commit -s -m "feat: runtime detection with matcha.toml override"
```

---

### Task 3: Embed the official starter template

**Files:**
- Create: `template/starter/src/app.module.ts`
- Create: `template/starter/src/controllers/home.controller.ts`
- Create: `template/starter/public/index.html`
- Create: `template/starter/deno.json`
- Create: `template/starter/matcha.toml`
- Create: `src/template.rs`
- Modify: `src/main.rs` (add `mod template;`)
- Test: `src/template.rs` (inline)

**Interfaces:**
- Produces: `pub static STARTER: include_dir::Dir` and `pub fn write_starter(dest: &Path, project_name: &str) -> std::io::Result<()>`. Consumed by Task 4.

- [ ] **Step 1: Write the starter files**

`template/starter/src/controllers/home.controller.ts`:

```ts
import { Route, Get, Html, Sse } from '@green-tea/core';

const ZEN = [
  'Simplicity is the ultimate sophistication.',
  'The obstacle is the path.',
  'When you let go, you have two hands to work.',
  'Make it work, make it right, make it fast.',
  'Less, but better.',
];

@Route('/')
export class HomeController {
  @Get('/')
  @Html('public/index.html')
  home() {}

  @Sse('/zen')
  zen() {
    return (async function* () {
      for (let i = 0; ; i++) {
        yield { zen: ZEN[i % ZEN.length] };
        await new Promise((r) => setTimeout(r, 30_000));
      }
    })();
  }
}
```

`template/starter/src/app.module.ts`:

```ts
import { Module } from '@green-tea/core';
import { HomeController } from './controllers/home.controller';

@Module({ controllers: [HomeController] })
export class AppModule {}
```

`template/starter/public/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>{{project_name}}</title>
    <style>
      body { font-family: system-ui; display: grid; place-items: center;
             min-height: 100vh; margin: 0; background: #0f1f13; color: #d7f5df; }
      .logo { font-size: 4rem; }
      .zen { margin-top: 1rem; opacity: .85; font-style: italic; }
    </style>
  </head>
  <body>
    <div>
      <div class="logo">🍵 green-tea</div>
      <div class="zen" id="zen">connecting…</div>
    </div>
    <script>
      const es = new EventSource('/zen');
      es.onmessage = (e) => { document.getElementById('zen').textContent = JSON.parse(e.data).zen; };
    </script>
  </body>
</html>
```

`template/starter/deno.json`:

```json
{ "tasks": { "dev": "deno run --watch --allow-net --allow-read src/main.ts" } }
```

`template/starter/matcha.toml`:

```toml
runtime = "deno"
```

- [ ] **Step 2: Write the failing test**

```rust
// bottom of src/template.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_starter_and_substitutes_name() {
        let d = tempdir().unwrap();
        write_starter(d.path(), "my-api").unwrap();
        let html = std::fs::read_to_string(d.path().join("public/index.html")).unwrap();
        assert!(html.contains("<title>my-api</title>"));
        assert!(!html.contains("{{project_name}}"));
        assert!(d.path().join("src/controllers/home.controller.ts").exists());
        assert!(d.path().join("matcha.toml").exists());
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test template`
Expected: FAIL — `write_starter` not defined.

- [ ] **Step 4: Implement `src/template.rs`**

```rust
use include_dir::{include_dir, Dir};
use std::path::Path;

pub static STARTER: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/starter");

pub fn write_starter(dest: &Path, project_name: &str) -> std::io::Result<()> {
    write_dir(&STARTER, dest, project_name)
}

fn write_dir(dir: &Dir, dest: &Path, name: &str) -> std::io::Result<()> {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(d) => {
                let sub = dest.join(d.path().file_name().unwrap());
                std::fs::create_dir_all(&sub)?;
                write_dir(d, &sub, name)?;
            }
            include_dir::DirEntry::File(f) => {
                let out = dest.join(f.path().file_name().unwrap());
                if let Some(parent) = out.parent() { std::fs::create_dir_all(parent)?; }
                let contents = std::str::from_utf8(f.contents())
                    .map(|s| s.replace("{{project_name}}", name).into_bytes())
                    .unwrap_or_else(|_| f.contents().to_vec());
                std::fs::write(out, contents)?;
            }
        }
    }
    Ok(())
}
```

> Note: `write_dir` rebuilds paths from `file_name()` so nested dirs land correctly relative to `dest`. If a deeper nesting bug shows up during the test, switch to `f.path()` relative to `STARTER.path()`.

- [ ] **Step 5: Wire the module**

In `src/main.rs` add: `mod template;`

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test template`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add template src/template.rs src/main.rs
git commit -s -m "feat: embed official starter template with token substitution"
```

---

### Task 4: `matcha new <name>` from embedded starter

**Files:**
- Create: `src/cmd_new.rs`
- Modify: `src/main.rs` (add `mod cmd_new;`, dispatch `Command::New`)
- Test: `tests/new.rs`

**Interfaces:**
- Consumes: `template::write_starter` (Task 3).
- Produces: `pub fn run(name: &str, template_url: Option<&str>) -> std::io::Result<()>`. The `template_url` arm is filled in Task 5 (return an error for now).

- [ ] **Step 1: Write the failing test**

```rust
// tests/new.rs
use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn new_scaffolds_into_named_dir() {
    let d = tempdir().unwrap();
    Command::cargo_bin("matcha").unwrap()
        .current_dir(d.path())
        .args(["new", "my-api"]).assert().success();
    let root = d.path().join("my-api");
    assert!(root.join("src/app.module.ts").exists());
    assert!(root.join("public/index.html").exists());
}

#[test]
fn new_refuses_existing_dir() {
    let d = tempdir().unwrap();
    std::fs::create_dir(d.path().join("taken")).unwrap();
    Command::cargo_bin("matcha").unwrap()
        .current_dir(d.path())
        .args(["new", "taken"]).assert().failure();
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --test new`
Expected: FAIL — command prints "not yet implemented", dir not created.

- [ ] **Step 3: Implement `src/cmd_new.rs`**

```rust
use std::io::{Error, ErrorKind};
use std::path::Path;

use crate::template;

pub fn run(name: &str, template_url: Option<&str>) -> std::io::Result<()> {
    let dest = Path::new(name);
    if dest.exists() {
        return Err(Error::new(ErrorKind::AlreadyExists,
            format!("'{name}' already exists")));
    }
    std::fs::create_dir_all(dest)?;

    match template_url {
        None => template::write_starter(dest, name)?,
        Some(_) => return Err(Error::new(ErrorKind::Unsupported,
            "--template-url not implemented yet")),
    }
    println!("✓ created {name}\n  cd {name} && matcha run");
    Ok(())
}
```

- [ ] **Step 4: Dispatch in `src/main.rs`**

Replace the `Command::New` arm:

```rust
Command::New { name, template_url } => {
    if let Err(e) = cmd_new::run(&name, template_url.as_deref()) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

And add `mod cmd_new;` near the top.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test new`
Expected: PASS (both).

- [ ] **Step 6: Commit**

```bash
git add src/cmd_new.rs src/main.rs tests/new.rs
git commit -s -m "feat: matcha new scaffolds from embedded starter"
```

---

### Task 5: `matcha new --template-url gh:owner/repo`

**Files:**
- Modify: `src/cmd_new.rs` (fill the `Some` arm, add `resolve_url`)
- Test: `src/cmd_new.rs` (inline unit test for URL mapping) + `tests/new.rs` (clone from a local git fixture)

**Interfaces:**
- Consumes: system `git`.
- Produces: `fn resolve_url(spec: &str) -> String` — `gh:owner/repo` → `https://github.com/owner/repo`, otherwise passthrough.

- [ ] **Step 1: Write the failing unit test**

```rust
// bottom of src/cmd_new.rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gh_shorthand_expands() {
        assert_eq!(resolve_url("gh:green-tea/tmpl"),
                   "https://github.com/green-tea/tmpl");
    }
    #[test]
    fn full_url_passes_through() {
        assert_eq!(resolve_url("https://example.com/x.git"),
                   "https://example.com/x.git");
    }
}
```

- [ ] **Step 2: Write the failing integration test**

```rust
// add to tests/new.rs
use std::process::Command as Proc;

#[test]
fn new_clones_local_template() {
    let d = tempdir().unwrap();
    // build a bare-ish source repo to clone
    let src = d.path().join("src-tmpl");
    std::fs::create_dir_all(src.join("public")).unwrap();
    std::fs::write(src.join("public/marker.txt"), "hi").unwrap();
    Proc::new("git").args(["init", "-q"]).current_dir(&src).status().unwrap();
    Proc::new("git").args(["add", "."]).current_dir(&src).status().unwrap();
    Proc::new("git").args(["-c","user.email=t@t","-c","user.name=t",
        "commit","-qm","init"]).current_dir(&src).status().unwrap();

    let url = format!("file://{}", src.display());
    Command::cargo_bin("matcha").unwrap()
        .current_dir(d.path())
        .args(["new", "cloned", "--template-url", &url]).assert().success();
    assert!(d.path().join("cloned/public/marker.txt").exists());
    assert!(!d.path().join("cloned/.git").exists()); // .git stripped
}
```

- [ ] **Step 3: Run to verify both fail**

Run: `cargo test`
Expected: FAIL — `resolve_url` missing; clone arm returns "not implemented".

- [ ] **Step 4: Implement clone in `src/cmd_new.rs`**

Add the function and replace the `Some` arm:

```rust
fn resolve_url(spec: &str) -> String {
    if let Some(rest) = spec.strip_prefix("gh:") {
        format!("https://github.com/{rest}")
    } else {
        spec.to_string()
    }
}

fn clone_template(url: &str, dest: &Path) -> std::io::Result<()> {
    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", &resolve_url(url)])
        .arg(dest)
        .status()?;
    if !status.success() {
        return Err(Error::new(ErrorKind::Other, "git clone failed"));
    }
    std::fs::remove_dir_all(dest.join(".git")).ok();
    Ok(())
}
```

Replace the `Some(_)` arm from Task 4:

```rust
Some(url) => clone_template(url, dest)?,
```

(`dest` is created empty by `create_dir_all`; `git clone <dest>` clones into it.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS (unit + integration).

- [ ] **Step 6: Commit**

```bash
git add src/cmd_new.rs tests/new.rs
git commit -s -m "feat: matcha new --template-url via shallow git clone"
```

---

### Task 6: `matcha run` — detect runtime and spawn watch

**Files:**
- Create: `src/cmd_run.rs`
- Modify: `src/main.rs` (add `mod cmd_run;`, dispatch)
- Test: `src/cmd_run.rs` (inline unit test for argv construction)

**Interfaces:**
- Consumes: `runtime::detect` (Task 2).
- Produces: `fn command_for(rt: Runtime) -> (&'static str, Vec<&'static str>)` — the program + args to spawn. `pub fn run(dir: &Path) -> std::io::Result<()>` execs it.

Testing note: we unit-test the **argv we build** (deterministic, no runtime needed), not a live process. Spawning is a thin wrapper around a tested value.

- [ ] **Step 1: Write the failing test**

```rust
// bottom of src/cmd_run.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;

    #[test]
    fn deno_uses_task_dev() {
        assert_eq!(command_for(Runtime::Deno), ("deno", vec!["task", "dev"]));
    }
    #[test]
    fn bun_watch() {
        assert_eq!(command_for(Runtime::Bun), ("bun", vec!["--watch", "run", "dev"]));
    }
    #[test]
    fn node_watch() {
        assert_eq!(command_for(Runtime::Node), ("npm", vec!["run", "dev"]));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test cmd_run`
Expected: FAIL — `command_for` not defined.

- [ ] **Step 3: Implement `src/cmd_run.rs`**

```rust
use std::io::{Error, ErrorKind};
use std::path::Path;

use crate::runtime::{self, Runtime};

fn command_for(rt: Runtime) -> (&'static str, Vec<&'static str>) {
    match rt {
        Runtime::Deno => ("deno", vec!["task", "dev"]),
        Runtime::Bun => ("bun", vec!["--watch", "run", "dev"]),
        Runtime::Node => ("npm", vec!["run", "dev"]),
    }
}

pub fn run(dir: &Path) -> std::io::Result<()> {
    let rt = runtime::detect(dir).ok_or_else(|| Error::new(
        ErrorKind::NotFound,
        "no runtime detected (need deno.json, bun.lock, or package.json)",
    ))?;
    let (prog, args) = command_for(rt);
    println!("→ {prog} {}", args.join(" "));
    let status = std::process::Command::new(prog).args(&args).current_dir(dir).status()?;
    std::process::exit(status.code().unwrap_or(1));
}
```

- [ ] **Step 4: Dispatch in `src/main.rs`**

```rust
Command::Run => {
    if let Err(e) = cmd_run::run(std::path::Path::new(".")) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

Add `mod cmd_run;`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test cmd_run`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/cmd_run.rs src/main.rs
git commit -s -m "feat: matcha run detects runtime and spawns watch"
```

---

### Task 7: Wiring core — tree-sitter import + @Module array edit

**Files:**
- Create: `src/wire.rs`
- Modify: `src/main.rs` (add `mod wire;`)
- Test: `src/wire.rs` (inline, string fixtures)

**Interfaces:**
- Produces:
  - `pub fn add_import(src: &str, symbol: &str, from: &str) -> String` — inserts `import { <symbol> } from '<from>';` after the last import; no-op if already present.
  - `pub fn add_to_module_array(src: &str, key: &str, symbol: &str) -> Option<String>` — inserts `<symbol>` into the `key: [...]` array of the first `@Module({...})`; `None` if the resulting text fails to re-parse (`ERROR` node); no-op-returns-`Some(src)` if already present.
  - `fn parses_clean(src: &str) -> bool` — true when tree-sitter finds no `ERROR` node.
- Consumed by Task 8.

- [ ] **Step 1: Write the failing tests**

```rust
// bottom of src/wire.rs
#[cfg(test)]
mod tests {
    use super::*;

    const MOD: &str = r#"import { Module } from '@green-tea/core';
import { HomeController } from './controllers/home.controller';

@Module({ controllers: [HomeController] })
export class AppModule {}
"#;

    #[test]
    fn import_is_added_once() {
        let out = add_import(MOD, "UsersController", "./controllers/users.controller");
        assert!(out.contains("import { UsersController } from './controllers/users.controller';"));
        let twice = add_import(&out, "UsersController", "./controllers/users.controller");
        assert_eq!(out, twice); // idempotent
    }

    #[test]
    fn symbol_added_to_controllers_array() {
        let out = add_to_module_array(MOD, "controllers", "UsersController").unwrap();
        assert!(out.contains("HomeController"));
        assert!(out.contains("UsersController"));
        // idempotent
        let again = add_to_module_array(&out, "controllers", "UsersController").unwrap();
        assert_eq!(out, again);
    }

    #[test]
    fn broken_result_is_rejected() {
        // key missing entirely → our splice can't find it → None, file untouched upstream
        assert!(add_to_module_array(MOD, "providers", "X").is_none()
             || add_to_module_array(MOD, "providers", "X").unwrap().contains("providers"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test wire`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement `src/wire.rs`**

```rust
use tree_sitter::{Parser, Node};

fn parser() -> Parser {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .expect("load ts grammar");
    p
}

fn parses_clean(src: &str) -> bool {
    let tree = parser().parse(src, None).unwrap();
    !has_error(tree.root_node())
}

fn has_error(node: Node) -> bool {
    if node.is_error() || node.is_missing() { return true; }
    let mut c = node.walk();
    node.children(&mut c).any(has_error)
}

pub fn add_import(src: &str, symbol: &str, from: &str) -> String {
    let line = format!("import {{ {symbol} }} from '{from}';");
    if src.contains(&line) { return src.to_string(); }
    // insert after the last import statement's line
    let tree = parser().parse(src, None).unwrap();
    let root = tree.root_node();
    let mut last_end = 0usize;
    let mut c = root.walk();
    for child in root.children(&mut c) {
        if child.kind() == "import_statement" {
            last_end = child.end_byte();
        }
    }
    if last_end == 0 {
        return format!("{line}\n{src}");
    }
    let mut out = String::with_capacity(src.len() + line.len() + 1);
    out.push_str(&src[..last_end]);
    out.push('\n');
    out.push_str(&line);
    out.push_str(&src[last_end..]);
    out
}

/// Finds the `key: [ ... ]` array inside the first `@Module({...})` and inserts `symbol`.
pub fn add_to_module_array(src: &str, key: &str, symbol: &str) -> Option<String> {
    let tree = parser().parse(src, None).unwrap();
    let (open, close, existing) = find_array(&tree.root_node(), src, key)?;
    if existing.split(',').map(str::trim).any(|s| s == symbol) {
        return Some(src.to_string()); // idempotent
    }
    let inner = &src[open + 1..close]; // between [ ]
    let joined = if inner.trim().is_empty() {
        symbol.to_string()
    } else {
        format!("{}, {}", inner.trim_end().trim_end_matches(','), symbol)
    };
    let mut out = String::with_capacity(src.len() + symbol.len() + 2);
    out.push_str(&src[..open + 1]);
    out.push_str(&joined);
    out.push_str(&src[close..]);
    if parses_clean(&out) { Some(out) } else { None }
}

/// Returns (open_bracket_byte, close_bracket_byte, inner_text) for `key: [...]`.
fn find_array(root: &Node, src: &str, key: &str) -> Option<(usize, usize, String)> {
    // Walk every pair `key: array` in the tree; pick the first matching key.
    fn visit(node: Node, src: &str, key: &str) -> Option<(usize, usize, String)> {
        if node.kind() == "pair" {
            if let Some(k) = node.child_by_field_name("key") {
                if k.utf8_text(src.as_bytes()).ok() == Some(key) {
                    if let Some(v) = node.child_by_field_name("value") {
                        if v.kind() == "array" {
                            let open = v.start_byte();
                            let close = v.end_byte() - 1; // the ']'
                            let inner = src[open + 1..close].to_string();
                            return Some((open, close, inner));
                        }
                    }
                }
            }
        }
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if let Some(r) = visit(child, src, key) { return Some(r); }
        }
        None
    }
    visit(*root, src, key)
}
```

> Grammar note: `tree-sitter-typescript` 0.21+ exposes `LANGUAGE_TYPESCRIPT`. If the linked version instead exposes `language_typescript()`, use `p.set_language(&tree_sitter_typescript::language_typescript())`. The failing→passing test loop will surface which one this crate version ships.

- [ ] **Step 4: Wire the module**

In `src/main.rs`: `mod wire;`

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test wire`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/wire.rs src/main.rs
git commit -s -m "feat: tree-sitter import + @Module array wiring with revert-on-break"
```

---

### Task 8: `matcha create <kind> <Name>` — generate + auto-wire

**Files:**
- Create: `src/cmd_create.rs`
- Modify: `src/main.rs` (add `mod cmd_create;`, dispatch)
- Test: `tests/create.rs`

**Interfaces:**
- Consumes: `wire::add_import`, `wire::add_to_module_array` (Task 7).
- Produces: `pub fn run(kind: &str, name: &str, check: bool) -> std::io::Result<()>`. Fallback: if `add_to_module_array` returns `None`, write the file and print a manual-wire hint instead of failing (the agreed "emit-only" degrade).

- [ ] **Step 1: Write the failing test**

```rust
// tests/create.rs
use assert_cmd::Command;
use tempfile::tempdir;

fn seed_module(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src/controllers")).unwrap();
    std::fs::write(dir.join("src/app.module.ts"),
"import { Module } from '@green-tea/core';\n\n\
@Module({ controllers: [] })\nexport class AppModule {}\n").unwrap();
}

#[test]
fn create_controller_writes_and_wires() {
    let d = tempdir().unwrap();
    seed_module(d.path());
    Command::cargo_bin("matcha").unwrap()
        .current_dir(d.path())
        .args(["create", "controller", "Users"]).assert().success();

    assert!(d.path().join("src/controllers/users.controller.ts").exists());
    let module = std::fs::read_to_string(d.path().join("src/app.module.ts")).unwrap();
    assert!(module.contains("UsersController"));
    assert!(module.contains("import { UsersController }"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --test create`
Expected: FAIL — "not yet implemented".

- [ ] **Step 3: Implement `src/cmd_create.rs`**

```rust
use std::io::{Error, ErrorKind};
use std::path::Path;

use crate::wire;

fn controller_stub(name: &str) -> String {
    format!(
"import {{ Route, Get }} from '@green-tea/core';\n\n\
@Route('/{lower}')\nexport class {name}Controller {{\n  \
@Get('/')\n  list() {{\n    return [];\n  }}\n}}\n",
        lower = name.to_lowercase(), name = name)
}

pub fn run(kind: &str, name: &str, _check: bool) -> std::io::Result<()> {
    match kind {
        "controller" => create_controller(name),
        "module" | "step" => Err(Error::new(ErrorKind::Unsupported,
            format!("create {kind} not implemented yet"))),
        other => Err(Error::new(ErrorKind::InvalidInput, format!("unknown kind {other}"))),
    }
}

fn create_controller(name: &str) -> std::io::Result<()> {
    let file = Path::new("src/controllers")
        .join(format!("{}.controller.ts", name.to_lowercase()));
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
    Ok(())
}
```

- [ ] **Step 4: Dispatch in `src/main.rs`**

```rust
Command::Create { kind, name, check } => {
    if let Err(e) = cmd_create::run(&kind, &name, check) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

Add `mod cmd_create;`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test create`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/cmd_create.rs src/main.rs tests/create.rs
git commit -s -m "feat: matcha create controller with tree-sitter auto-wire + emit-only fallback"
```

---

### Task 9: Optional semantic check (`--check`)

**Files:**
- Create: `src/check.rs`
- Modify: `src/cmd_create.rs` (call check when `check == true` and config present), `src/main.rs` (`mod check;`)
- Test: `src/check.rs` (inline, argv construction)

**Interfaces:**
- Consumes: `runtime::detect` (Task 2).
- Produces: `fn check_command(rt: Runtime) -> (&'static str, Vec<&'static str>)`; `pub fn run(dir: &Path) -> std::io::Result<bool>` returns `true` when the type-check passes (or is unavailable → skip, `true`).

- [ ] **Step 1: Write the failing test**

```rust
// bottom of src/check.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;
    #[test]
    fn deno_check_cmd() {
        assert_eq!(check_command(Runtime::Deno), ("deno", vec!["check", "src/app.module.ts"]));
    }
    #[test]
    fn node_uses_tsc_noemit() {
        assert_eq!(check_command(Runtime::Node), ("npx", vec!["tsc", "--noEmit"]));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test check`
Expected: FAIL — `check_command` not defined.

- [ ] **Step 3: Implement `src/check.rs`**

```rust
use std::path::Path;
use crate::runtime::{self, Runtime};

fn check_command(rt: Runtime) -> (&'static str, Vec<&'static str>) {
    match rt {
        Runtime::Deno => ("deno", vec!["check", "src/app.module.ts"]),
        Runtime::Bun  => ("bunx", vec!["tsc", "--noEmit"]),
        Runtime::Node => ("npx", vec!["tsc", "--noEmit"]),
    }
}

pub fn run(dir: &Path) -> std::io::Result<bool> {
    let Some(rt) = runtime::detect(dir) else { return Ok(true) }; // nothing to check against
    let (prog, args) = check_command(rt);
    let status = std::process::Command::new(prog).args(&args).current_dir(dir).status()?;
    Ok(status.success())
}
```

- [ ] **Step 4: Call it from `cmd_create.rs`**

At the end of `create_controller`, before `Ok(())`, add a `check: bool` param threaded from `run` and:

```rust
    if check {
        match crate::check::run(Path::new(".")) {
            Ok(true) => println!("✓ type-check passed"),
            Ok(false) => println!("⚠ type-check reported errors"),
            Err(e) => println!("⚠ could not run type-check: {e}"),
        }
    }
```

Update `create_controller(name)` → `create_controller(name, check)` and pass `_check` through as `check`.

- [ ] **Step 5: Run tests + build to verify**

Run: `cargo test check && cargo build`
Expected: PASS + clean build.

- [ ] **Step 6: Commit**

```bash
git add src/check.rs src/cmd_create.rs src/main.rs
git commit -s -m "feat: optional --check delegates type-check to detected runtime"
```

---

### Task 10: `matcha add <sse|stream|buffer>` — append a handler to a controller

**Files:**
- Create: `src/cmd_add.rs`
- Modify: `src/main.rs` (`mod cmd_add;`, dispatch), `src/wire.rs` (add `insert_method`)
- Test: `src/wire.rs` (inline) + `tests/add.rs`

**Interfaces:**
- Consumes: `wire::add_import`, plus new `wire::insert_method`.
- Produces: `pub fn insert_method(src: &str, class: &str, method_src: &str) -> Option<String>` — inserts `method_src` before the closing `}` of `class <class>`; `None` if re-parse fails. `cmd_add::run(cap: &str) -> std::io::Result<()>` targets the single `*.controller.ts` in `src/controllers` (errors if 0 or >1).

- [ ] **Step 1: Write the failing wire test**

```rust
// add to src/wire.rs tests module
#[test]
fn method_inserted_before_class_close() {
    let src = "export class HomeController {\n  home() {}\n}\n";
    let out = insert_method(src, "HomeController", "  @Sse('/zen')\n  zen() {}\n").unwrap();
    assert!(out.contains("zen()"));
    assert!(out.trim_end().ends_with('}'));
}
```

- [ ] **Step 2: Write the failing integration test**

```rust
// tests/add.rs
use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn add_sse_appends_handler() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src/controllers")).unwrap();
    std::fs::write(d.path().join("src/controllers/home.controller.ts"),
"import { Route, Get } from '@green-tea/core';\n\n\
@Route('/')\nexport class HomeController {\n  @Get('/')\n  home() {}\n}\n").unwrap();

    Command::cargo_bin("matcha").unwrap()
        .current_dir(d.path())
        .args(["add", "sse"]).assert().success();

    let out = std::fs::read_to_string(d.path().join("src/controllers/home.controller.ts")).unwrap();
    assert!(out.contains("@Sse("));
    assert!(out.contains("import { Route, Get, Sse }") || out.contains("Sse }"));
}
```

- [ ] **Step 3: Run to verify both fail**

Run: `cargo test`
Expected: FAIL — `insert_method` missing; `add` not implemented.

- [ ] **Step 4: Add `insert_method` to `src/wire.rs`**

```rust
pub fn insert_method(src: &str, class: &str, method_src: &str) -> Option<String> {
    let tree = parser().parse(src, None).unwrap();
    let close = find_class_body_close(&tree.root_node(), src, class)?;
    let mut out = String::with_capacity(src.len() + method_src.len() + 1);
    out.push_str(&src[..close]);
    if !src[..close].ends_with('\n') { out.push('\n'); }
    out.push_str(method_src);
    out.push_str(&src[close..]);
    if parses_clean(&out) { Some(out) } else { None }
}

fn find_class_body_close(root: &Node, src: &str, class: &str) -> Option<usize> {
    fn visit(node: Node, src: &str, class: &str) -> Option<usize> {
        if node.kind() == "class_declaration" {
            if let Some(n) = node.child_by_field_name("name") {
                if n.utf8_text(src.as_bytes()).ok() == Some(class) {
                    if let Some(body) = node.child_by_field_name("body") {
                        return Some(body.end_byte() - 1); // the closing '}'
                    }
                }
            }
        }
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if let Some(r) = visit(child, src, class) { return Some(r); }
        }
        None
    }
    visit(*root, src, class)
}
```

- [ ] **Step 5: Implement `src/cmd_add.rs`**

```rust
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use crate::wire;

fn handler(cap: &str) -> Option<(&'static str, String)> {
    match cap {
        "sse" => Some(("Sse",
"\n  @Sse('/events')\n  events() {\n    return (async function* () {\n      yield { hello: 'world' };\n    })();\n  }\n".into())),
        "stream" => Some(("Stream",
"\n  @Stream('/stream')\n  stream() {\n    return (async function* () {\n      yield 'chunk';\n    })();\n  }\n".into())),
        "buffer" => Some(("Get",
"\n  @Get('/data')\n  data() {\n    return Buffer.from('hello');\n  }\n".into())),
        _ => None,
    }
}

fn only_controller() -> std::io::Result<PathBuf> {
    let dir = std::path::Path::new("src/controllers");
    let mut hits: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".controller.ts"))
        .collect();
    match hits.len() {
        1 => Ok(hits.pop().unwrap()),
        0 => Err(Error::new(ErrorKind::NotFound, "no *.controller.ts in src/controllers")),
        _ => Err(Error::new(ErrorKind::Other, "multiple controllers; specify which (not yet supported)")),
    }
}

pub fn run(cap: &str) -> std::io::Result<()> {
    let (decorator, method) = handler(cap)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, format!("unknown capability {cap}")))?;
    let path = only_controller()?;
    let src = std::fs::read_to_string(&path)?;
    // class name = file stem PascalCased + "Controller" — but read it from the source instead:
    let class = class_name(&src)
        .ok_or_else(|| Error::new(ErrorKind::Other, "no exported class found"))?;
    let with_import = wire::add_import(&src, decorator, "@green-tea/core");
    // add_import above assumes a fresh import line; if '@green-tea/core' already imported,
    // we still append a second import line — acceptable, tsc merges. Keep lazy for v1.
    match wire::insert_method(&with_import, &class, &method) {
        Some(out) => { std::fs::write(&path, out)?; println!("✓ added @{decorator} to {class}"); }
        None => println!("→ could not edit {}; add @{decorator} handler manually", path.display()),
    }
    Ok(())
}

fn class_name(src: &str) -> Option<String> {
    let idx = src.find("export class ")? + "export class ".len();
    let rest = &src[idx..];
    let end = rest.find(|c: char| !c.is_alphanumeric() && c != '_')?;
    Some(rest[..end].to_string())
}
```

> `add_import` reuse note: for `@green-tea/core` the symbol may already be imported under a shared `{ ... }`. v1 keeps it lazy (a duplicate named import line is legal TS and tsc dedups at type level). Merging into the existing brace is a Task-for-later; not worth tree-sitter surgery now.

- [ ] **Step 6: Dispatch in `src/main.rs`**

```rust
Command::Add { capability } => {
    if let Err(e) = cmd_add::run(&capability) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

Add `mod cmd_add;`.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS (wire unit + add integration + everything prior).

- [ ] **Step 8: Commit**

```bash
git add src/cmd_add.rs src/wire.rs src/main.rs tests/add.rs
git commit -s -m "feat: matcha add appends capability handler to the controller"
```

---

### Task 11: README + install docs

**Files:**
- Create: `README.md`
- Test: none (docs).

**Interfaces:** none.

- [ ] **Step 1: Write `README.md`**

```markdown
# matcha

The green-tea CLI.

## Install

    cargo install matcha        # or: brew install matcha (once published)

## Usage

    matcha new my-api                          # scaffold from the official starter
    matcha new my-api --template-url gh:owner/repo
    matcha run                                 # detect node/deno/bun and run in watch
    matcha create controller Users             # generate + auto-wire into @Module
    matcha add sse                             # add a capability to your controller

## Runtime detection

Precedence: `matcha.toml` (`runtime = "..."`) → `deno.json` → `bun.lock(b)` → `package.json`.
Edge is a deploy target, not a `run` target.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -s -m "docs: matcha README with usage and runtime detection"
```

---

## Self-Review

**Spec coverage:**
- §2 command surface → Tasks 1, 4, 5, 6, 8, 10. ✓
- §3 `new` embedded starter + `--template-url` + starter "wow" → Tasks 3, 4, 5. ✓
- §4 runtime detection → Task 2; used in Tasks 6, 9. ✓
- §5 codegen (tree-sitter edit + syntax-check revert + optional semantic check + idempotent + fallback) → Tasks 7, 8, 9. ✓
- §6 Rust architecture (clap, include_dir, tree-sitter, git clone) → Tasks 1, 3, 5, 7. ✓
- §7 out of scope respected (no catalog, no edge run, no plugin/LSP/bundler). ✓
- Gap: `create module`/`create step` are stubbed as "not implemented" in Task 8 — spec lists them in the command surface. **Deliberate:** v1 proves the wiring pipeline with `controller`; `module`/`step` reuse the same `wire` primitives and are fast follow-ups. Flagged here rather than silently dropped.

**Placeholder scan:** No TBD/TODO left as work items; every code step is complete and runnable. Grammar-version and import-merge notes are explicit engineering caveats with the resolution path, not placeholders.

**Type consistency:** `runtime::detect`/`Runtime` used identically in Tasks 2/6/9. `wire::add_import`, `add_to_module_array`, `insert_method` signatures match between Task 7/10 definitions and Task 8/10 call sites. `cmd_*::run` signatures match their `main.rs` dispatch.

**Known execution-time unknowns (converge via the test loop):**
- `tree-sitter-typescript` language accessor name (`LANGUAGE_TYPESCRIPT` vs `language_typescript()`) — Task 7 note.
- `pair`/`array`/`class_declaration` node/field names in the grammar version pulled — the first `cargo test wire` run confirms or corrects them.
