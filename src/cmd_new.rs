use std::io::{Error, ErrorKind};
use std::path::Path;

use crate::template;

pub fn run(name: &str, template_url: Option<&str>) -> std::io::Result<()> {
    let dest = Path::new(name);
    if dest.exists() {
        return Err(Error::new(ErrorKind::AlreadyExists,
            format!("'{name}' already exists")));
    }

    match template_url {
        None => {
            std::fs::create_dir_all(dest)?;
            template::write_starter(dest, name)?;
        }
        Some(url) => clone_template(url, dest)?,
    }
    println!("✓ created {name}\n  cd {name} && matcha run");
    Ok(())
}

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
        return Err(Error::other("git clone failed"));
    }
    std::fs::remove_dir_all(dest.join(".git")).ok();
    Ok(())
}

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
