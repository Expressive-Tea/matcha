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
