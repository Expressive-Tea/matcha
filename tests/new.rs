use assert_cmd::Command;
use std::process::Command as Proc;
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
