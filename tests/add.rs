use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn add_sse_appends_handler() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src/controllers")).unwrap();
    std::fs::write(
        d.path().join("src/controllers/home.controller.ts"),
        "import { Route, Get } from '@green-tea/core';\n\n\
@Route('/')\nexport class HomeController {\n  @Get('/')\n  home() {}\n}\n",
    )
    .unwrap();

    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["add", "sse"])
        .assert()
        .success();

    let out = std::fs::read_to_string(d.path().join("src/controllers/home.controller.ts")).unwrap();
    assert!(out.contains("@Sse("));
    assert!(out.contains("import { Route, Get, Sse }") || out.contains("Sse }"));
}

#[test]
fn add_errors_when_no_controller() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src/controllers")).unwrap();
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["add", "sse"])
        .assert()
        .failure();
}

#[test]
fn add_sse_twice_is_idempotent() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src/controllers")).unwrap();
    std::fs::write(d.path().join("src/controllers/home.controller.ts"),
"import { Route, Get } from '@green-tea/core';\n\n@Route('/')\nexport class HomeController {\n  @Get('/')\n  home() {}\n}\n").unwrap();
    let run = || {
        Command::cargo_bin("matcha")
            .unwrap()
            .current_dir(d.path())
            .args(["add", "sse"])
            .assert()
            .success()
    };
    run();
    run();
    let out = std::fs::read_to_string(d.path().join("src/controllers/home.controller.ts")).unwrap();
    // exactly one Sse handler, exactly one @green-tea/core import line
    assert_eq!(out.matches("@Sse(").count(), 1);
    assert_eq!(out.matches("from '@green-tea/core'").count(), 1);
}

#[test]
fn add_errors_when_multiple_controllers_and_writes_nothing() {
    let d = tempdir().unwrap();
    let cdir = d.path().join("src/controllers");
    std::fs::create_dir_all(&cdir).unwrap();
    let original = "export class X {}\n";
    for n in ["a", "b"] {
        std::fs::write(cdir.join(format!("{n}.controller.ts")), original).unwrap();
    }
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["add", "sse"])
        .assert()
        .failure();
    // no file was modified
    assert_eq!(
        std::fs::read_to_string(cdir.join("a.controller.ts")).unwrap(),
        original
    );
    assert_eq!(
        std::fs::read_to_string(cdir.join("b.controller.ts")).unwrap(),
        original
    );
}
