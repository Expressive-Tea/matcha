//! Where the application graph is composed: the file holding the `createApp`
//! call and exporting the `app` it returns.
//!
//! `src/app.ts` is what `matcha new` scaffolds, and it exists as its own file
//! precisely so the graph can be imported without binding a port. Projects
//! scaffolded before that split — and hand-written ones that never took it —
//! keep the whole thing in `src/main.ts`, so that stays a candidate.

use std::path::{Path, PathBuf};

/// Candidate entry files, most specific first.
const CANDIDATES: [&str; 2] = ["src/app.ts", "src/main.ts"];

/// The first candidate that exists under `dir`, or `None` when neither does.
pub fn find(dir: &Path) -> Option<PathBuf> {
    CANDIDATES.iter().map(|c| dir.join(c)).find(|p| p.exists())
}

/// The candidate list as prose, for the error a caller prints when `find`
/// returns `None`.
pub fn candidates() -> String {
    CANDIDATES.join(" or ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "").unwrap();
    }

    #[test]
    fn prefers_app_over_main() {
        let d = tempdir().unwrap();
        touch(d.path(), "src/main.ts");
        touch(d.path(), "src/app.ts");
        assert_eq!(find(d.path()), Some(d.path().join("src/app.ts")));
    }

    #[test]
    fn falls_back_to_main() {
        let d = tempdir().unwrap();
        touch(d.path(), "src/main.ts");
        assert_eq!(find(d.path()), Some(d.path().join("src/main.ts")));
    }

    #[test]
    fn none_when_neither_exists() {
        let d = tempdir().unwrap();
        assert_eq!(find(d.path()), None);
    }
}
