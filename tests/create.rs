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

/// A project scaffolded today composes its graph in `src/app.ts`, so that is
/// where a new module has to be registered. `src/main.ts` only serves, and
/// wiring a module into it would put the module in a file that never calls
/// `createApp` — silently, since `add_to_createapp_modules` would simply find
/// nothing to edit and the CLI would print its "add it manually" hint.
#[test]
fn create_module_prefers_app_ts_over_main_ts() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src")).unwrap();
    std::fs::write(
        d.path().join("src/app.ts"),
        "import { createApp } from '@green-tea/core';\nimport { AppModule } from './app.module';\n\nexport const app = createApp({ modules: [AppModule] });\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("src/main.ts"),
        "import { app } from './app';\n\napp.listen(3000);\n",
    )
    .unwrap();

    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(d.path())
        .args(["create", "module", "Users"])
        .assert()
        .success();

    let app_ts = std::fs::read_to_string(d.path().join("src/app.ts")).unwrap();
    assert!(app_ts.contains("AppModule, UsersModule"));
    assert!(app_ts.contains("import { UsersModule }"));

    let main = std::fs::read_to_string(d.path().join("src/main.ts")).unwrap();
    assert!(!main.contains("UsersModule"), "main.ts was edited: {main}");
}

fn create(dir: &std::path::Path, kind: &str, name: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("matcha")
        .unwrap()
        .current_dir(dir)
        .args(["create", kind, name])
        .assert()
}

#[test]
fn create_step_keeps_camel_case_in_the_token_and_reads_kebab_input() {
    let d = tempdir().unwrap();
    seed_module(d.path());
    create(d.path(), "step", "current-user").success();
    let step = std::fs::read_to_string(d.path().join("src/steps/currentuser.step.ts")).unwrap();
    assert!(step.contains("class CurrentUserStep"), "{step}");
    assert!(step.contains("provides: 'currentUser'"), "{step}");
    assert!(step.contains("return { currentUser: null }"), "{step}");
}

#[test]
fn create_refuses_a_token_core_already_provides() {
    let d = tempdir().unwrap();
    seed_module(d.path());
    create(d.path(), "provider", "Logger")
        .failure()
        .stderr(predicates::str::contains("core already provides"));
    create(d.path(), "step", "req").failure();
    assert!(!d.path().join("src/providers/logger.provider.ts").exists());
}

#[test]
fn create_refuses_a_name_that_cannot_be_a_class() {
    let d = tempdir().unwrap();
    seed_module(d.path());
    create(d.path(), "controller", "2fast").failure();
    create(d.path(), "controller", "us.ers").failure();
}

#[test]
fn create_does_not_overwrite_an_existing_file() {
    let d = tempdir().unwrap();
    seed_module(d.path());
    create(d.path(), "controller", "Users").success();
    let file = d.path().join("src/controllers/users.controller.ts");
    std::fs::write(&file, "// edited\n").unwrap();
    create(d.path(), "controller", "Users")
        .failure()
        .stderr(predicates::str::contains("already exists"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "// edited\n");
}
