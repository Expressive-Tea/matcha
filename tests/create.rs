use assert_cmd::Command;
use tempfile::tempdir;

fn seed_module(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src/controllers")).unwrap();
    std::fs::write(dir.join("src/app.module.ts"),
"import { Module } from '@green-tea/core';\n\n\
@Module({ controllers: [] })\nexport class AppModule {}\n").unwrap();
}

#[test]
fn create_controller_writes_and_wires() {
    let d = tempdir().unwrap();
    seed_module(d.path());
    Command::cargo_bin("matcha").unwrap()
        .current_dir(d.path())
        .args(["create", "controller", "Users"]).assert().success();

    assert!(d.path().join("src/controllers/users.controller.ts").exists());
    let module = std::fs::read_to_string(d.path().join("src/app.module.ts")).unwrap();
    assert!(module.contains("UsersController"));
    assert!(module.contains("import { UsersController }"));
}
