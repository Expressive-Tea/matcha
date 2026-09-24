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

fn files_under(dir: &std::path::Path) -> Vec<String> {
    fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out)
            } else {
                out.push(p.strip_prefix(base).unwrap().display().to_string())
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[test]
fn package_jsr_writes_the_tree_into_dir() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "Plugin Algo",
            "--package=algo-pkg",
            "--scope",
            "@acme",
        ])
        .assert()
        .success();
    let pkg = d.path().join("algo-pkg");
    assert_eq!(
        files_under(&pkg),
        [
            ".gitignore",
            "CHANGELOG.md",
            "LICENSE",
            "README.md",
            "deno.json",
            "package.json",
            "src/index.ts",
            "test/plugin-algo.test.ts"
        ]
    );
    let deno = std::fs::read_to_string(pkg.join("deno.json")).unwrap();
    assert!(deno.contains("\"name\": \"@acme/plugin-algo\""), "{deno}");
    assert!(deno.contains("npm:@green-tea/core@26.9.0-beta.2"), "{deno}");
    // Without it `deno test` needs a `deno install` first, and then fails to
    // type-check `node:assert` for want of @types/node.
    assert!(deno.contains("\"nodeModulesDir\": \"auto\""), "{deno}");
    let pj = std::fs::read_to_string(pkg.join("package.json")).unwrap();
    assert!(pj.contains("\"private\": true"), "{pj}");
    assert!(pj.contains("\"devDependencies\""), "{pj}");
    assert!(
        !pj.contains("\"dependencies\""),
        "core is a type-only dependency: {pj}"
    );
    let readme = std::fs::read_to_string(pkg.join("README.md")).unwrap();
    assert!(readme.contains("JSR is recommended"), "banner");
    for runtime in ["| Node |", "| Deno |", "| Bun |", "| workerd (edge) |"] {
        assert!(readme.contains(runtime), "{runtime} row");
    }
}

#[test]
fn package_both_adds_the_npm_build() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "algo",
            "--package=p",
            "--scope",
            "acme",
            "--registry",
            "both",
        ])
        .assert()
        .success();
    let pkg = d.path().join("p");
    assert!(pkg.join("tsconfig.json").exists());
    let pj = std::fs::read_to_string(pkg.join("package.json")).unwrap();
    assert!(
        pj.contains("\"name\": \"@acme/green-tea-algo\""),
        "suggested name is the default: {pj}"
    );
    assert!(!pj.contains("\"private\""), "{pj}");
    assert!(pj.contains("\"prepublishOnly\": \"npm run build\""), "{pj}");
    let readme = std::fs::read_to_string(pkg.join("README.md")).unwrap();
    assert!(readme.contains("npm i @acme/green-tea-algo"), "{readme}");
}

#[test]
fn both_package_json_is_esm_only() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "algo",
            "--package=p",
            "--scope",
            "acme",
            "--registry",
            "both",
        ])
        .assert()
        .success();
    let pj = std::fs::read_to_string(d.path().join("p/package.json")).unwrap();
    assert!(pj.contains("\"type\": \"module\""), "{pj}");
    assert!(!pj.contains("require"), "{pj}");
    assert!(!pj.contains(".cjs"), "{pj}");
}

#[test]
fn package_accepts_any_npm_name_and_notes_the_convention() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "algo",
            "--package=p",
            "--scope",
            "acme",
            "--registry",
            "both",
            "--npm-name",
            "algo-plugin",
        ])
        .assert()
        .success()
        .stdout(contains("green-tea-<x> is the naming convention"));
    let pj = std::fs::read_to_string(d.path().join("p/package.json")).unwrap();
    assert!(pj.contains("\"name\": \"algo-plugin\""), "{pj}");
}

#[test]
fn package_refuses_a_non_empty_directory_and_writes_nothing() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("keep.txt"), "x").unwrap();
    matcha(d.path())
        .args(["create", "plugin", "algo", "--package", "--scope", "acme"])
        .assert()
        .failure()
        .stderr(contains("a plugin package needs an empty directory"));
    assert_eq!(files_under(d.path()), ["keep.txt"]);
}

#[test]
fn package_in_an_empty_current_directory() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args(["create", "plugin", "algo", "--package", "--scope", "acme"])
        .assert()
        .success();
    assert!(d.path().join("deno.json").exists());
}

#[test]
fn package_off_a_tty_needs_a_scope() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args(["create", "plugin", "algo", "--package=p"])
        .assert()
        .failure()
        .stderr(contains("--scope"));
    assert!(!d.path().join("p").exists());
}

#[test]
fn package_rejects_a_bad_scope_or_npm_name() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "algo",
            "--package=p",
            "--scope",
            "Ac Me",
        ])
        .assert()
        .failure()
        .stderr(contains("scope"));
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "algo",
            "--package=q",
            "--scope",
            "acme",
            "--registry",
            "both",
            "--npm-name",
            "Not Valid",
        ])
        .assert()
        .failure()
        .stderr(contains("npm name"));
}

#[test]
fn a_name_whose_factory_would_be_a_reserved_word_writes_nothing() {
    for name in ["delete", "class", "default", "new", "import"] {
        let d = project(APP);
        matcha(d.path())
            .args(["create", "plugin", name])
            .assert()
            .failure()
            .stderr(contains("reserved word"));
        assert!(!d.path().join("plugins").exists(), "{name}");
        assert_eq!(
            std::fs::read_to_string(d.path().join("src/app.ts")).unwrap(),
            APP
        );
    }
}

#[test]
fn a_package_whose_factory_would_be_a_reserved_word_writes_nothing() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "delete",
            "--package=p",
            "--scope",
            "acme",
        ])
        .assert()
        .failure()
        .stderr(contains("reserved word"));
    assert!(!d.path().join("p").exists());
}

/// `create app` → `createApp`, and `app` → `app`: both already exist in the entry, and a second
/// binding of the same name is a SyntaxError that would break the app, not just the plugin.
#[test]
fn in_app_refuses_a_factory_name_the_entry_already_uses() {
    for name in ["create app", "app"] {
        let d = project(APP);
        matcha(d.path())
            .args(["create", "plugin", name])
            .assert()
            .failure()
            .stderr(contains("already"));
        assert!(!d.path().join("plugins").exists(), "{name}");
        assert_eq!(
            std::fs::read_to_string(d.path().join("src/app.ts")).unwrap(),
            APP,
            "{name}"
        );
    }
}

/// The scaffolded app.ts imports from '@green-tea/core' and './app.module'. Words inside those
/// strings are not bindings, so `core`, `tea` and `module` are free names for a plugin.
#[test]
fn words_inside_strings_and_comments_are_not_collisions() {
    let app = "import { createApp } from '@green-tea/core';\nimport { AppModule } from './app.module';\n\n// the green tea app\nexport const app = createApp({ modules: [AppModule] });\n";
    for name in ["core", "tea", "module", "green"] {
        let d = project(app);
        matcha(d.path())
            .args(["create", "plugin", name])
            .assert()
            .success();
        assert!(
            d.path().join(format!("plugins/{name}/index.ts")).exists(),
            "{name}"
        );
    }
}

#[test]
fn package_engines_need_the_node_that_strips_types() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args(["create", "plugin", "algo", "--package=p", "--scope", "acme"])
        .assert()
        .success();
    let pj = std::fs::read_to_string(d.path().join("p/package.json")).unwrap();
    // `node --test test/*.test.ts` needs unflagged type stripping, which is 22.18+.
    assert!(pj.contains("\"node\": \">=22.18\""), "{pj}");
}

#[test]
fn the_generated_test_asserts_the_token_is_in_the_graph() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args(["create", "plugin", "algo", "--package=p", "--scope", "acme"])
        .assert()
        .success();
    let t = std::fs::read_to_string(d.path().join("p/test/algo.test.ts")).unwrap();
    assert!(t.contains("app.graph()"), "{t}");
    assert!(t.contains("provides.includes('algo')"), "{t}");
}

#[test]
fn package_into_a_path_that_is_a_file_names_it() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("taken"), "x").unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "algo",
            "--package=taken",
            "--scope",
            "acme",
        ])
        .assert()
        .failure()
        .stderr(contains("taken is a file"));
}

#[test]
fn the_empty_directory_error_names_the_directory() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("busy")).unwrap();
    std::fs::write(d.path().join("busy/keep.txt"), "x").unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "algo",
            "--package=busy",
            "--scope",
            "acme",
        ])
        .assert()
        .failure()
        .stderr(contains("busy: a plugin package needs an empty directory"));
}

#[cfg(unix)]
#[test]
fn in_app_leaves_nothing_behind_when_the_entry_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;
    let d = project(APP);
    let entry = d.path().join("src/app.ts");
    std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o444)).unwrap();
    matcha(d.path())
        .args(["create", "plugin", "algo"])
        .assert()
        .failure();
    std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        !d.path().join("plugins/algo").exists(),
        "the plugin folder is rolled back"
    );
    assert_eq!(std::fs::read_to_string(&entry).unwrap(), APP);
}

#[test]
fn folder_dot_and_leading_dot_slash_give_clean_imports() {
    let d = project(APP);
    matcha(d.path())
        .args(["create", "plugin", "algo", "--folder", "."])
        .assert()
        .success();
    assert!(d.path().join("algo/index.ts").exists());
    let app = std::fs::read_to_string(d.path().join("src/app.ts")).unwrap();
    assert!(app.contains("from '../algo/index'"), "{app}");

    let d = project(APP);
    matcha(d.path())
        .args(["create", "plugin", "algo", "--folder", "./plugins/"])
        .assert()
        .success();
    let app = std::fs::read_to_string(d.path().join("src/app.ts")).unwrap();
    assert!(app.contains("from '../plugins/algo/index'"), "{app}");
}

#[test]
fn plugin_flags_on_another_kind_are_an_error() {
    let d = project(APP);
    matcha(d.path())
        .args(["create", "controller", "Users", "--scope", "acme"])
        .assert()
        .failure()
        .stderr(contains("only apply to create plugin"));
    assert!(!d.path().join("src/controllers").exists());
}

#[test]
fn flags_for_the_other_mode_are_named_as_ignored() {
    let d = project(APP);
    matcha(d.path())
        .args(["create", "plugin", "algo", "--scope", "acme"])
        .assert()
        .success()
        .stdout(contains("--scope only applies to --package; ignored"));

    let d = tempdir().unwrap();
    matcha(d.path())
        .args([
            "create",
            "plugin",
            "algo",
            "--package",
            "--scope",
            "acme",
            "--folder",
            "x",
            "--check",
        ])
        .assert()
        .success()
        .stdout(contains(
            "--folder only applies to an in-app plugin; ignored",
        ))
        .stdout(contains(
            "--check only applies to an in-app plugin; ignored",
        ));
}

/// `--package algo` used to take `algo` as the directory. It now needs `--package=DIR`, so a
/// bare word after `--package` is the plugin's name.
#[test]
fn a_word_after_package_is_the_name_not_the_directory() {
    let d = tempdir().unwrap();
    matcha(d.path())
        .args(["create", "plugin", "--package", "algo", "--scope", "acme"])
        .assert()
        .success();
    let deno = std::fs::read_to_string(d.path().join("deno.json")).unwrap();
    assert!(deno.contains("\"name\": \"@acme/algo\""), "{deno}");
}

#[test]
fn scopes_jsr_would_refuse_are_refused() {
    for scope in ["-", "-acme", "a", "abcdefghijklmnopqrstu"] {
        let d = tempdir().unwrap();
        matcha(d.path())
            // `=` so a leading hyphen is a value, not another flag.
            .args([
                "create",
                "plugin",
                "algo",
                "--package",
                &format!("--scope={scope}"),
            ])
            .assert()
            .failure()
            .stderr(contains("is not a JSR scope"));
        assert!(!d.path().join("deno.json").exists(), "{scope}");
    }
}

#[test]
fn packages_ship_a_license_and_node_types() {
    for registry in ["jsr", "both"] {
        let d = tempdir().unwrap();
        matcha(d.path())
            .args([
                "create",
                "plugin",
                "algo",
                "--package",
                "--scope",
                "acme",
                "--registry",
                registry,
            ])
            .assert()
            .success();
        let license = std::fs::read_to_string(d.path().join("LICENSE")).unwrap();
        assert!(license.starts_with("MIT License"), "{registry}");
        let pj = std::fs::read_to_string(d.path().join("package.json")).unwrap();
        assert!(pj.contains("\"license\": \"MIT\""), "{registry}: {pj}");
        assert!(pj.contains("\"@types/node\""), "{registry}: {pj}");
    }
}
