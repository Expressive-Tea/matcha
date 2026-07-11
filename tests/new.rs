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

#[test]
fn new_with_template_url_stub_leaves_no_dir() {
    let d = tempdir().unwrap();
    Command::cargo_bin("matcha").unwrap()
        .current_dir(d.path())
        .args(["new", "demo", "--template-url", "gh:x/y"]).assert().failure();
    assert!(!d.path().join("demo").exists());
}
