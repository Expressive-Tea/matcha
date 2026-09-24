//! `matcha create plugin`: an in-app plugin folder by default, or, with
//! `--package` or a yes, a package of its own.

use std::io::{self, BufRead, Error, ErrorKind, Write};
use std::path::{Component, Path};

use crate::ask::Ask;
use crate::{entry, naming, plugin_files, wire};

pub struct Opts {
    pub name: Option<String>,
    /// `None`: not asked for. `Some(None)`: `--package` with no directory.
    pub package: Option<Option<String>>,
    pub folder: Option<String>,
    pub scope: Option<String>,
    pub registry: Option<String>,
    pub npm_name: Option<String>,
    pub check: bool,
}

pub fn run(opts: Opts) -> io::Result<()> {
    run_with(opts, &mut Ask::stdio(), Path::new("."))
}

pub fn run_with<R: BufRead, W: Write>(
    opts: Opts,
    ask: &mut Ask<R, W>,
    root: &Path,
) -> io::Result<()> {
    let package = match opts.package {
        Some(_) => true,
        None => ask.yes_no("Separate it as a package?", false)?,
    };
    if package {
        package_mode(opts, ask, root)
    } else {
        in_app(opts, ask, root)
    }
}

fn invalid(msg: String) -> Error {
    Error::new(ErrorKind::InvalidInput, msg)
}

fn resolve_slug<R: BufRead, W: Write>(
    name: Option<String>,
    ask: &mut Ask<R, W>,
) -> io::Result<String> {
    let raw = match name {
        Some(n) => n,
        None => ask.text("Plugin name?", None, "the NAME argument")?,
    };
    naming::slug(&raw).ok_or_else(|| {
        invalid(format!(
            "'{raw}' does not make a plugin name: start with a letter, and use letters, digits, spaces or hyphens"
        ))
    })
}

/// Relative, and never climbing out: `..` or an absolute path would put the
/// plugin outside the project it is being wired into.
fn inside_project(folder: &str) -> bool {
    !folder.is_empty()
        && Path::new(folder)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn in_app<R: BufRead, W: Write>(opts: Opts, ask: &mut Ask<R, W>, root: &Path) -> io::Result<()> {
    let Some(entry_path) = entry::find(root) else {
        return Err(invalid(format!(
            "an in-app plugin is wired into createApp, and there is no {} here; run this in a project, or pass --package for a package of its own",
            entry::candidates()
        )));
    };
    let folder = match opts.folder {
        Some(f) => f,
        None => ask.text("Folder?", Some("plugins"), "--folder")?,
    };
    let folder = folder.trim_end_matches('/').to_string();
    if !inside_project(&folder) {
        return Err(invalid(format!(
            "the plugin folder has to be inside the project, got '{folder}'"
        )));
    }
    let slug = resolve_slug(opts.name, ask)?;

    let dir = root.join(&folder).join(&slug);
    if dir.exists() {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            format!(
                "{} already exists",
                Path::new(&folder).join(&slug).display()
            ),
        ));
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("index.ts"), plugin_files::factory(&slug))?;
    println!(
        "✓ {}",
        Path::new(&folder).join(&slug).join("index.ts").display()
    );

    let fun = naming::camel(&slug);
    let call = format!("{fun}()");
    // Both entry candidates live in src/, so the plugin folder is one level up.
    let from = format!("../{folder}/{slug}/index");
    let src = std::fs::read_to_string(&entry_path)?;
    let imported = wire::add_import(&src, &fun, &from);
    match wire::add_to_createapp(&imported, "plugins", &call) {
        Some(wired) => {
            std::fs::write(&entry_path, wired)?;
            println!("✓ registered {call} in createApp");
        }
        None => println!(
            "→ could not auto-register; add {call} to createApp plugins[] in {}, and import {{ {fun} }} from '{from}'",
            entry_path.strip_prefix(root).unwrap_or(&entry_path).display()
        ),
    }
    crate::cmd_create::maybe_check(opts.check)
}

fn package_mode<R: BufRead, W: Write>(
    _opts: Opts,
    _ask: &mut Ask<R, W>,
    _root: &Path,
) -> io::Result<()> {
    Err(invalid("package mode is not built yet".into()))
}
