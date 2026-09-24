use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

const APP: &str = "import { createApp } from '@green-tea/core';\nimport { AppModule } from './app.module';\n\nexport const app = createApp({ modules: [AppModule] });\n";

fn project(app_ts: &str) -> tempfile::TempDir {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("src")).unwrap();
    std::fs::write(d.path().join("src/app.ts"), app_ts).unwrap();
    d
}

fn matcha(dir: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("matcha").unwrap();
    c.current_dir(dir);
    c
}

#[test]
fn in_app_is_the_default_and_wires_into_create_app() {
    let d = project(APP);
    matcha(d.path())
        .args(["create", "plugin", "Plugin Algo"])
        .assert()
        .success();

    let index = std::fs::read_to_string(d.path().join("plugins/plugin-algo/index.ts")).unwrap();
    assert!(index.contains("export function pluginAlgo("));
    assert!(index.contains("options.provides ?? 'plugin-algo'"));
    let app = std::fs::read_to_string(d.path().join("src/app.ts")).unwrap();
    assert!(
        app.contains("import { pluginAlgo } from '../plugins/plugin-algo/index';"),
        "{app}"
    );
    assert!(app.contains("plugins: [pluginAlgo()]"), "{app}");
}

#[test]
fn in_app_appends_to_an_existing_plugins_array() {
    let d = project("import { createApp } from '@green-tea/core';\n\nexport const app = createApp({ modules: [], plugins: [other()] });\n");
    matcha(d.path())
        .args(["create", "plugin", "algo"])
        .assert()
        .success();
    let app = std::fs::read_to_string(d.path().join("src/app.ts")).unwrap();
    assert!(app.contains("plugins: [other(), algo()]"), "{app}");
}

#[test]
fn in_app_honours_folder() {
    let d = project(APP);
    matcha(d.path())
        .args(["create", "plugin", "algo", "--folder", "lib/plugins"])
        .assert()
        .success();
    assert!(d.path().join("lib/plugins/algo/index.ts").exists());
    let app = std::fs::read_to_string(d.path().join("src/app.ts")).unwrap();
    assert!(app.contains("'../lib/plugins/algo/index'"), "{app}");
}

#[test]
fn in_app_leaves_a_non_array_plugins_alone_and_says_what_to_add() {
    let original = "import { createApp } from '@green-tea/core';\n\nexport const app = createApp({ modules: [], plugins: PLUGINS });\n";
    let d = project(original);
    matcha(d.path())
        .args(["create", "plugin", "algo"])
        .assert()
        .success()
        .stdout(contains("add algo() to createApp plugins[]"));
    assert_eq!(
        std::fs::read_to_string(d.path().join("src/app.ts")).unwrap(),
        original
    );
    assert!(d.path().join("plugins/algo/index.ts").exists());
}

#[test]
fn in_app_refuses_an_existing_folder() {
    let d = project(APP);
    std::fs::create_dir_all(d.path().join("plugins/algo")).unwrap();
    matcha(d.path())
        .args(["create", "plugin", "algo"])
        .assert()
        .failure()
        .stderr(contains("already exists"));
    assert_eq!(
        std::fs::read_to_string(d.path().join("src/app.ts")).unwrap(),
        APP
    );
}

#[test]
fn in_app_refuses_a_folder_outside_the_project() {
    let d = project(APP);
    for folder in ["../outside", "/tmp/elsewhere"] {
        matcha(d.path())
            .args(["create", "plugin", "algo", "--folder", folder])
            .assert()
            .failure()
            .stderr(contains("inside the project"));
    }
}

#[test]
fn in_app_needs_a_project() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args(["create", "plugin", "algo"])
        .assert()
        .failure()
        .stderr(contains("src/app.ts"));
}

#[test]
fn a_bad_name_writes_nothing() {
    let d = project(APP);
    matcha(d.path())
        .args(["create", "plugin", "🍵"])
        .assert()
        .failure()
        .stderr(contains("does not make a plugin name"));
    assert!(!d.path().join("plugins").exists());
}

#[test]
fn off_a_tty_a_missing_name_is_an_error_not_a_hang() {
    let d = project(APP);
    matcha(d.path())
        .args(["create", "plugin"])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .failure()
        .stderr(contains("NAME"));
}

#[test]
fn other_kinds_still_require_a_name() {
    let d = project(APP);
    matcha(d.path())
        .args(["create", "controller"])
        .assert()
        .failure()
        .stderr(contains("needs a name"));
}
