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
    // A NAME given on the command line is checked before any question.
    if let Some(name) = &opts.name {
        check_name(name)?;
    }
    let package = match opts.package {
        Some(_) => true,
        None => ask.yes_no("Separate it as a package?", false)?,
    };
    if package {
        for (flag, given) in [("--folder", opts.folder.is_some()), ("--check", opts.check)] {
            if given {
                println!("note: {flag} only applies to an in-app plugin; ignored");
            }
        }
        package_mode(opts, ask, root)
    } else {
        for (flag, given) in [
            ("--scope", opts.scope.is_some()),
            ("--registry", opts.registry.is_some()),
            ("--npm-name", opts.npm_name.is_some()),
        ] {
            if given {
                println!("note: {flag} only applies to --package; ignored");
            }
        }
        in_app(opts, ask, root)
    }
}

fn invalid(msg: String) -> Error {
    Error::new(ErrorKind::InvalidInput, msg)
}

/// The slug for `raw`, or why it cannot be one: not a name, or a factory that
/// would be a reserved word.
fn check_name(raw: &str) -> io::Result<String> {
    let slug = naming::slug(raw).ok_or_else(|| {
        invalid(format!(
            "'{raw}' does not make a plugin name: start with a letter, and use letters, digits, spaces or hyphens"
        ))
    })?;
    let fun = naming::camel(&slug);
    if naming::is_reserved(&fun) {
        return Err(invalid(format!(
            "'{raw}' would make the factory `{fun}`, which is a reserved word in TypeScript; pick another name"
        )));
    }
    Ok(slug)
}

fn resolve_slug<R: BufRead, W: Write>(
    name: Option<String>,
    ask: &mut Ask<R, W>,
) -> io::Result<String> {
    let raw = match name {
        Some(n) => n,
        None => ask.text("Plugin name?", None, "the NAME argument")?,
    };
    check_name(&raw)
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
    if !inside_project(&folder) {
        return Err(invalid(format!(
            "the plugin folder has to be inside the project, got '{folder}'"
        )));
    }
    // `.`, `./plugins` and `plugins/` all mean what they say without leaving a
    // `./` or a trailing slash in the import path.
    let folder: Vec<String> = Path::new(&folder)
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let folder = folder.join("/");
    let slug = resolve_slug(opts.name, ask)?;

    let fun = naming::camel(&slug);
    let src = std::fs::read_to_string(&entry_path)?;
    let entry_shown = entry_path
        .strip_prefix(root)
        .unwrap_or(&entry_path)
        .display()
        .to_string();
    // A second binding of a name the entry already has is a SyntaxError, and it
    // would break the app rather than only the new plugin.
    if wire::binds(&src, &fun) {
        return Err(invalid(format!(
            "{entry_shown} already uses `{fun}`, so the plugin's factory would collide with it; pick another name"
        )));
    }

    let shown = Path::new(&folder).join(&slug);
    let dir = root.join(&shown);
    if dir.exists() {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            format!("{} already exists", shown.display()),
        ));
    }

    let call = format!("{fun}()");
    // Both entry candidates live in src/, so the plugin folder is one level up.
    let from = if folder.is_empty() {
        format!("../{slug}/index")
    } else {
        format!("../{folder}/{slug}/index")
    };
    let wired = wire::add_to_createapp(&wire::add_import(&src, &fun, &from), "plugins", &call);

    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("index.ts"), plugin_files::factory(&slug))?;
    println!("✓ {}", shown.join("index.ts").display());

    match wired {
        Some(wired) => {
            // The plugin folder and its registration land together or not at
            // all: a folder left behind would make the rerun say "already exists".
            if let Err(e) = std::fs::write(&entry_path, wired) {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(Error::new(
                    e.kind(),
                    format!(
                        "could not write {entry_shown} ({e}); removed {} again",
                        shown.display()
                    ),
                ));
            }
            println!("✓ registered {call} in createApp");
        }
        None => println!(
            "→ could not auto-register; add {call} to createApp plugins[] in {entry_shown}, and import {{ {fun} }} from '{from}'"
        ),
    }
    crate::cmd_create::maybe_check(opts.check)
}

const EMPTY_DIR: &str = "a plugin package needs an empty directory, so the package stays self-contained — pass --package=<dir> or run it in an empty folder";
const NPM_CONVENTION: &str =
    "green-tea-<x> is the naming convention; the plugin listing uses it to find green-tea plugins.";

/// JSR's scope rule: 2 to 20 characters, lowercase letters, digits and
/// hyphens, and not starting with a hyphen.
fn valid_scope(s: &str) -> bool {
    (2..=20).contains(&s.len())
        && !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Writes every file, reporting each; on a failure, the error names the files
/// already written, so a half-filled directory is not a mystery on the rerun.
fn write_all(dir: &Path, root: &Path, files: &[(String, String)]) -> io::Result<()> {
    let mut written: Vec<&str> = Vec::new();
    for (rel, content) in files {
        let path = dir.join(rel);
        let result = std::fs::create_dir_all(path.parent().unwrap())
            .and_then(|()| std::fs::write(&path, content));
        if let Err(e) = result {
            let so_far = if written.is_empty() {
                "none".to_string()
            } else {
                written.join(", ")
            };
            return Err(Error::new(
                e.kind(),
                format!("could not write {rel} ({e}); already written: {so_far}"),
            ));
        }
        println!("✓ {}", path.strip_prefix(root).unwrap_or(&path).display());
        written.push(rel);
    }
    Ok(())
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
    let dir_shown = dir.strip_prefix(root).unwrap_or(&dir).display().to_string();
    let dir_shown = if dir_shown.is_empty() {
        ".".to_string()
    } else {
        dir_shown
    };
    if dir.is_file() {
        return Err(invalid(format!("{dir_shown} is a file; {EMPTY_DIR}")));
    }
    if dir.exists() && std::fs::read_dir(&dir)?.next().is_some() {
        return Err(invalid(format!("{dir_shown}: {EMPTY_DIR}")));
    }

    let slug = resolve_slug(opts.name, ask)?;
    let scope = match opts.scope {
        Some(s) => s,
        None => ask.text("JSR scope?", None, "--scope")?,
    };
    let scope = scope.trim_start_matches('@').to_string();
    if !valid_scope(&scope) {
        return Err(invalid(format!(
            "'{scope}' is not a JSR scope: 2 to 20 lowercase letters, digits or hyphens, not starting with a hyphen"
        )));
    }

    let registry = match opts.registry {
        Some(r) => r,
        None => {
            ask.say(&format!("\n{}\n", plugin_files::JSR_BANNER))?;
            ask.choice(
                "Publish to: (1) JSR only [default]  (2) JSR + npm",
                &[("1", "jsr"), ("2", "both")],
                "jsr",
                "--registry",
            )?
            .to_string()
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
    write_all(&dir, root, &plugin_files::package(&package))?;
    println!(
        "→ next: fill in the Runtimes table in README.md, then `deno test` and `deno publish`"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn opts(name: Option<&str>) -> Opts {
        Opts {
            name: name.map(str::to_string),
            package: None,
            folder: None,
            scope: None,
            registry: None,
            npm_name: None,
            check: false,
        }
    }

    /// A NAME passed on the command line is checked before any question: nobody should answer
    /// "package?" and "folder?" to learn the name was never going to work. With no answers
    /// scripted, asking anything would end in an end-of-input error instead.
    #[test]
    fn a_bad_name_fails_before_any_question() {
        let mut ask = Ask::new(true, Cursor::new(Vec::new()), Vec::new());
        let err = run_with(opts(Some("🍵")), &mut ask, Path::new("/nonexistent")).unwrap_err();
        assert!(
            err.to_string().contains("does not make a plugin name"),
            "{err}"
        );
    }

    #[test]
    fn write_all_reports_what_it_wrote_before_failing() {
        let d = tempfile::tempdir().unwrap();
        let files = vec![
            ("a".to_string(), "x".to_string()),
            ("a/b".to_string(), "y".to_string()),
        ];
        let err = write_all(d.path(), d.path(), &files).unwrap_err();
        assert!(err.to_string().contains("already written: a"), "{err}");
    }
}
