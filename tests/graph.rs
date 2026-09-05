use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

/// Both introspection commands need a file that exports the app. Without one
/// there is nothing to import, and the failure has to name the fix — a stack
/// trace from a runtime that could not resolve a module does not.
#[test]
fn graph_without_an_entry_says_what_is_missing() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("package.json"), "{}").unwrap();
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .arg("graph")
        .assert()
        .failure()
        .stderr(contains("--entry"))
        .stderr(contains("export const app = createApp"));
}

#[test]
fn explain_rejects_an_entry_that_does_not_exist() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("package.json"), "{}").unwrap();
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["explain", "/users", "--entry", "src/nope.ts"])
        .assert()
        .failure()
        .stderr(contains("no such entry"));
}

/// Runtime detection comes after the entry is found, so a project with an entry
/// and no manifest is a distinct failure with its own message.
#[test]
fn graph_without_a_runtime_says_so() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src")).unwrap();
    std::fs::write(d.path().join("src/app.ts"), "export const app = {};\n").unwrap();
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .arg("graph")
        .assert()
        .failure()
        .stderr(contains("no runtime detected"));
}

#[test]
fn graph_rejects_an_unknown_format() {
    let d = tempdir().unwrap();
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["graph", "--format", "svg"])
        .assert()
        .failure();
}

/// openapi shares graph's entry resolution, so it shares its failure modes —
/// including the one that has to name the fix.
#[test]
fn openapi_without_an_entry_says_what_is_missing() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("package.json"), "{}").unwrap();
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .arg("openapi")
        .assert()
        .failure()
        .stderr(contains("--entry"));
}
