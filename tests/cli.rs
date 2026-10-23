use assert_cmd::Command;
use predicates::prelude::*;

fn bin() -> Command {
    let mut cmd = Command::cargo_bin("structured").expect("binary");
    cmd.env("XDG_CONFIG_HOME", "/tmp/structured-cli-empty");
    cmd
}

#[test]
fn help_lists_task_commands() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("tui"))
        .stdout(predicate::str::contains("login"));
}

#[test]
fn today_without_sign_in_is_auth_json_when_piped() {
    bin()
        .args(["today", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("\"code\":\"AUTH\""))
        .stderr(predicate::str::contains("structured login"));
}

#[test]
fn no_args_without_a_tty_prints_help() {
    bin()
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Usage:"))
        .stderr(predicate::str::contains("tui"));
}

#[test]
fn delete_requires_yes() {
    bin()
        .args(["delete", "task-id", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("pass --yes"));
}
