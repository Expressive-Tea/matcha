use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn help_lists_all_subcommands() {
    Command::cargo_bin("matcha")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("new"))
        .stdout(contains("run"))
        .stdout(contains("create"))
        .stdout(contains("add"))
        .stdout(contains("graph"))
        .stdout(contains("explain"))
        .stdout(contains("openapi"))
        .stdout(contains("doctor"));
}

/// clap's accepted values and the values `run` dispatches on come from one list
/// per command now. This is what proves the wiring reached clap — a regression
/// would take the parser and the dispatcher out of step, which shows up as a
/// value the CLI accepts and then refuses.
#[test]
fn create_and_add_advertise_every_value_they_accept() {
    for (command, values) in [
        (
            "create",
            vec!["module", "controller", "step", "provider", "plugin"],
        ),
        ("add", vec!["sse", "stream", "buffer", "ws", "upload"]),
    ] {
        let mut assertion = Command::cargo_bin("matcha")
            .unwrap()
            .args([command, "--help"])
            .assert()
            .success();
        for value in values {
            assertion = assertion.stdout(contains(value));
        }
    }
}

#[test]
fn new_requires_a_name() {
    Command::cargo_bin("matcha")
        .unwrap()
        .arg("new")
        .assert()
        .failure(); // missing <name>
}
