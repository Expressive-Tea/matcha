use include_dir::{include_dir, Dir};
use std::path::Path;

pub static STARTER: Dir = include_dir!("$CARGO_MANIFEST_DIR/template/starter");

#[allow(dead_code)]
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
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let contents = std::str::from_utf8(f.contents())
                    .map(|s| s.replace("{{project_name}}", name).into_bytes())
                    .unwrap_or_else(|_| f.contents().to_vec());
                std::fs::write(out, contents)?;
            }
        }
    }
    Ok(())
}

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
