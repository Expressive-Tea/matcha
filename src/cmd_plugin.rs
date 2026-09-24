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

const EMPTY_DIR: &str = "a plugin package needs an empty directory, so the package stays self-contained — pass --package <dir> or run it in an empty folder";
const NPM_CONVENTION: &str =
    "green-tea-<x> is the naming convention; the plugin listing uses it to find green-tea plugins.";

/// JSR's scope rule: lowercase letters, digits and hyphens.
fn valid_scope(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// npm's package name rule, closely enough to catch a typo: optional `@scope/`, lowercase.
fn valid_npm_name(s: &str) -> bool {
    let part = |p: &str| {
        !p.is_empty()
            && !p.starts_with(['.', '_'])
            && p.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-._~".contains(c))
    };
    match s.strip_prefix('@').and_then(|rest| rest.split_once('/')) {
        Some((scope, name)) => part(scope) && part(name),
        None => !s.starts_with('@') && part(s),
    }
}

fn package_mode<R: BufRead, W: Write>(
    opts: Opts,
    ask: &mut Ask<R, W>,
    root: &Path,
) -> io::Result<()> {
    // The directory is checked before any question: a person should not answer
    // four questions to be told the folder was never going to work.
    let dir = match &opts.package {
        Some(Some(d)) => root.join(d),
        _ => root.to_path_buf(),
    };
    if dir.exists() && std::fs::read_dir(&dir)?.next().is_some() {
        return Err(invalid(EMPTY_DIR.into()));
    }

    let slug = resolve_slug(opts.name, ask)?;
    let scope = match opts.scope {
        Some(s) => s,
        None => ask.text("JSR scope?", None, "--scope")?,
    };
    let scope = scope.trim_start_matches('@').to_string();
    if !valid_scope(&scope) {
        return Err(invalid(format!(
            "'{scope}' is not a JSR scope: use lowercase letters, digits and hyphens"
        )));
    }

    let registry = match opts.registry {
        Some(r) => r,
        None => {
            ask.say(&format!("\n{}\n", plugin_files::JSR_BANNER))?;
            match ask
                .text(
                    "Publish to: (1) JSR only  (2) JSR + npm",
                    Some("1"),
                    "--registry",
                )?
                .as_str()
            {
                "2" | "both" => "both".to_string(),
                _ => "jsr".to_string(),
            }
        }
    };

    let npm = if registry == "both" {
        let suggested = format!("@{scope}/green-tea-{slug}");
        let name = match opts.npm_name {
            Some(n) => n,
            // The default in parentheses is the suggestion; Enter takes it.
            None => ask.text("npm package name?", Some(&suggested), "--npm-name")?,
        };
        if !valid_npm_name(&name) {
            return Err(invalid(format!("'{name}' is not a valid npm name")));
        }
        let unscoped = name.rsplit('/').next().unwrap_or(&name);
        if !unscoped.starts_with("green-tea-") {
            println!("note: {NPM_CONVENTION}");
        }
        Some(name)
    } else {
        None
    };

    let package = plugin_files::Package { slug, scope, npm };
    for (rel, content) in plugin_files::package(&package) {
        let path = dir.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, content)?;
        println!("✓ {}", path.strip_prefix(root).unwrap_or(&path).display());
    }
    println!(
        "→ next: fill in the Runtimes table in README.md, then `deno test` and `deno publish`"
    );
    Ok(())
}
