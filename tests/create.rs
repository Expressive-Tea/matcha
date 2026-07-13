use assert_cmd::Command;
use tempfile::tempdir;

fn seed_module(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src/controllers")).unwrap();
    std::fs::write(
        dir.join("src/app.module.ts"),
        "import { Module } from '@green-tea/core';\n\n\
@Module({ controllers: [] })\nexport class AppModule {}\n",
    )
    .unwrap();
}

#[test]
fn create_controller_writes_and_wires() {
    let d = tempdir().unwrap();
    seed_module(d.path());
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["create", "controller", "Users"])
        .assert()
        .success();

    assert!(d
        .path()
        .join("src/controllers/users.controller.ts")
        .exists());
    let module = std::fs::read_to_string(d.path().join("src/app.module.ts")).unwrap();
    assert!(module.contains("UsersController"));
    assert!(module.contains("import { UsersController }"));
}

#[test]
fn create_step_writes_and_wires() {
    let d = tempdir().unwrap();
    seed_module(d.path()); // writes src/app.module.ts with `@Module({ controllers: [] })`
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["create", "step", "Pasito"])
        .assert()
        .success();
    assert!(d.path().join("src/steps/pasito.step.ts").exists());
    let m = std::fs::read_to_string(d.path().join("src/app.module.ts")).unwrap();
    assert!(m.contains("PasitoStep"));
    assert!(m.contains("import { PasitoStep }"));
    assert!(m.contains("steps: [PasitoStep]"));
}

#[test]
fn create_provider_writes_and_wires() {
    let d = tempdir().unwrap();
    seed_module(d.path());
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["create", "provider", "Config"])
        .assert()
        .success();
    assert!(d.path().join("src/providers/config.provider.ts").exists());
    let m = std::fs::read_to_string(d.path().join("src/app.module.ts")).unwrap();
    assert!(m.contains("providers: [ConfigProvider]"));
    assert!(m.contains("import { ConfigProvider }"));
}

#[test]
fn create_module_writes_and_registers_in_createapp() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src")).unwrap();
    std::fs::write(
        d.path().join("src/main.ts"),
        "import { createApp } from '@green-tea/core';\nimport { AppModule } from './app.module';\n\nconst app = createApp({ modules: [AppModule] });\n",
    )
    .unwrap();
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["create", "module", "Users"])
        .assert()
        .success();
    assert!(d.path().join("src/users.module.ts").exists());
    let main = std::fs::read_to_string(d.path().join("src/main.ts")).unwrap();
    assert!(main.contains("AppModule, UsersModule"));
    assert!(main.contains("import { UsersModule }"));
}
