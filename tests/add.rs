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
    // The generated stream is resumable, which takes three symbols, not one:
    // `@Sse` to declare the route, `sse()` to tag each event with an id, and
    // `@header` to read the id the browser echoes back on reconnect.
    for symbol in ["Sse", "sse", "header"] {
        assert!(
            out.contains(&format!("{symbol},")) || out.contains(&format!("{symbol} }}")),
            "{symbol} not imported:\n{out}"
        );
    }
    assert!(out.contains("last-event-id"));
    assert!(out.contains("id: String(next)"));
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

fn seed(dir: &std::path::Path, name: &str) {
    std::fs::create_dir_all(dir.join("src/controllers")).unwrap();
    std::fs::write(
        dir.join(format!("src/controllers/{name}.controller.ts")),
        format!(
            "import {{ Route, Get }} from '@green-tea/core';\n\n@Route('/')\nexport class {}Controller {{\n  @Get('/')\n  home() {{}}\n}}\n",
            name.to_uppercase()
        ),
    )
    .unwrap();
}

#[test]
fn add_ws_wires_the_duplex_handler_and_names_its_peer_dependency() {
    let d = tempdir().unwrap();
    seed(d.path(), "home");
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["add", "ws"])
        .assert()
        .success()
        // `ws` is lazy-required, so its absence is a runtime error rather than a
        // boot failure — the moment to mention it is when the handler is written.
        .stdout(predicates::str::contains("npm i ws"));

    let out = std::fs::read_to_string(d.path().join("src/controllers/home.controller.ts")).unwrap();
    assert!(out.contains("@Ws('/echo')"));
    assert!(out.contains("@inbound()"));
    assert!(out.contains("channel<string>()"));
}

#[test]
fn add_upload_wires_multipart_and_names_busboy() {
    let d = tempdir().unwrap();
    seed(d.path(), "home");
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["add", "upload"])
        .assert()
        .success()
        .stdout(predicates::str::contains("npm i busboy"));

    let out = std::fs::read_to_string(d.path().join("src/controllers/home.controller.ts")).unwrap();
    assert!(out.contains("@Post('/upload')"));
    assert!(out.contains("MultipartBody"));
}

/// With more than one controller the CLI used to refuse and stop. It still
/// refuses to guess, but `--controller` is now the way through.
#[test]
fn add_targets_the_named_controller_when_several_exist() {
    let d = tempdir().unwrap();
    seed(d.path(), "home");
    seed(d.path(), "users");

    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["add", "sse"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--controller"));

    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args([
            "add",
            "sse",
            "--controller",
            "src/controllers/users.controller.ts",
        ])
        .assert()
        .success();

    let users =
        std::fs::read_to_string(d.path().join("src/controllers/users.controller.ts")).unwrap();
    assert!(users.contains("@Sse("));
    let home =
        std::fs::read_to_string(d.path().join("src/controllers/home.controller.ts")).unwrap();
    assert!(
        !home.contains("@Sse("),
        "the other controller was edited too"
    );
}

#[test]
fn add_rejects_a_controller_path_that_does_not_exist() {
    let d = tempdir().unwrap();
    seed(d.path(), "home");
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args([
            "add",
            "sse",
            "--controller",
            "src/controllers/nope.controller.ts",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no such controller"));
}
