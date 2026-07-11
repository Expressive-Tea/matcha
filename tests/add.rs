use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn add_sse_appends_handler() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src/controllers")).unwrap();
    std::fs::write(d.path().join("src/controllers/home.controller.ts"),
"import { Route, Get } from '@green-tea/core';\n\n\
@Route('/')\nexport class HomeController {\n  @Get('/')\n  home() {}\n}\n").unwrap();

    Command::cargo_bin("matcha").unwrap()
        .current_dir(d.path())
        .args(["add", "sse"]).assert().success();

    let out = std::fs::read_to_string(d.path().join("src/controllers/home.controller.ts")).unwrap();
    assert!(out.contains("@Sse("));
    assert!(out.contains("import { Route, Get, Sse }") || out.contains("Sse }"));
}
