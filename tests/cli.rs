use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn help_lists_all_subcommands() {
    Command::cargo_bin("matcha").unwrap()
        .arg("--help").assert().success()
        .stdout(contains("new")).stdout(contains("run"))
        .stdout(contains("create")).stdout(contains("add"));
}

#[test]
fn new_requires_a_name() {
    Command::cargo_bin("matcha").unwrap()
        .arg("new").assert().failure(); // missing <name>
}
