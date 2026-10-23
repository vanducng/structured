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

mod mock;

use mock::Mock;
use serde_json::{Value, json};

const RID: &str = "2304478a-0636-4a2d-b9a1-903c247b0305";

fn fixture(tool: &str, args: &Value) -> Value {
    let plain = json!({"id": "task-1", "title": "Export", "day": null, "is_in_inbox": true,
        "subtasks": [{"id": "s1", "title": "Map fields", "completed_at": null}]});
    let occurrence = json!({"id": "occ-1", "recurring_id": RID, "is_recurring": true, "title": "Run",
        "day": "2026-10-23", "is_all_day": true, "subtasks": [{"id": "r1", "title": "Stretch", "completed_at": null}]});
    match tool {
        "get_inbox" => json!({"tasks": [plain]}),
        "get_today" | "get_tasks_for_day" | "list_tasks" => json!({"tasks": [occurrence]}),
        "update_task" | "create_task" => {
            json!({"task": {"id": "task-1", "title": "Export", "subtasks": args.get("subtasks").cloned().unwrap_or(json!([]))}})
        }
        _ => json!({"ok": true}),
    }
}

fn stdout_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout is one JSON object")
}

#[test]
fn task_json_carries_a_ref_and_subtasks() {
    let mock = Mock::start(fixture);
    let output = mock.cmd().args(["today", "--json"]).output().unwrap();
    assert!(output.status.success());
    let task = &stdout_json(&output)["tasks"][0];
    assert_eq!(task["ref"], format!("{RID}@2026-10-23"));
    assert_eq!(task["subtasks"][0]["title"], "Stretch");
    assert_eq!(task["is_completed"], false);
}

#[test]
fn subtask_add_keeps_existing_ids_and_appends_new_ones() {
    let mock = Mock::start(fixture);
    let output = mock
        .cmd()
        .args(["subtask", "add", "task-1", "Backfill", "Dry run", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let args = mock.call("update_task");
    assert_eq!(args["id"], "task-1");
    assert_eq!(
        args["subtasks"],
        json!([{"id": "s1", "title": "Map fields", "completed_at": null}, {"title": "Backfill"}, {"title": "Dry run"}])
    );
    assert!(args.get("update_action").is_none());
    assert_eq!(
        stdout_json(&output)["result"]["task"]["subtasks"][2]["title"],
        "Dry run"
    );
}

#[test]
fn ticking_an_occurrence_subtask_is_completion_only() {
    let mock = Mock::start(fixture);
    let target = format!("{RID}@2026-10-23");
    mock.cmd()
        .args(["subtask", "done", &target, "1", "--json"])
        .assert()
        .success();
    assert_eq!(mock.call("get_tasks_for_day")["day"], "2026-10-23");
    let args = mock.call("update_task");
    assert_eq!(args["recurring_id"], RID);
    assert_eq!(args["day"], "2026-10-23");
    assert!(args["subtasks"][0]["completed_at"].is_string());
    assert!(args.get("update_action").is_none());
}

#[test]
fn renaming_an_occurrence_subtask_stays_on_that_day() {
    let mock = Mock::start(fixture);
    let target = format!("{RID}@2026-10-23");
    mock.cmd()
        .args(["sub", "edit", &target, "1", "Warm up", "--json"])
        .assert()
        .success();
    let args = mock.call("update_task");
    assert_eq!(args["update_action"], "only_this");
    assert_eq!(
        args["subtasks"][0],
        json!({"id": "r1", "title": "Warm up", "completed_at": null})
    );
}

#[test]
fn occurrence_edits_need_a_scope() {
    let mock = Mock::start(fixture);
    let target = format!("{RID}@2026-10-23");
    mock.cmd()
        .args(["edit", &target, "--title", "Long run", "--json"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--scope series"));
    assert!(mock.calls().is_empty());
    let output = mock
        .cmd()
        .args([
            "edit", &target, "--title", "Long run", "--scope", "series", "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(stdout_json(&output)["result"]["scope"], "series");
    mock.cmd()
        .args([
            "edit",
            RID,
            "--day",
            "2026-10-23",
            "--move-day",
            "2026-10-24",
            "--scope",
            "day",
            "--json",
        ])
        .assert()
        .success();
    let edits: Vec<Value> = mock
        .calls()
        .into_iter()
        .filter(|c| c["tool"] == "update_task")
        .collect();
    assert_eq!(edits[0]["arguments"]["update_action"], "all");
    assert_eq!(edits[1]["arguments"]["update_action"], "only_this");
    assert_eq!(edits[1]["arguments"]["new_day"], "2026-10-24");
    assert_eq!(edits[1]["arguments"]["day"], "2026-10-23");
    mock.cmd()
        .args(["edit", &target, "--inbox", "--scope", "series", "--json"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot move"));
}

#[test]
fn every_write_returns_one_envelope_with_a_ref() {
    let mock = Mock::start(fixture);
    let added = stdout_json(
        &mock
            .cmd()
            .args(["add", "Export", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(added["result"]["operation"], "created");
    assert_eq!(added["result"]["ref"], "task-1");
    assert_eq!(added["result"]["task"]["ref"], "task-1");
    let deleted = stdout_json(
        &mock
            .cmd()
            .args(["delete", "task-1", "--yes", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        deleted["result"],
        json!({"operation": "deleted", "scope": "task", "ref": "task-1"})
    );
}

#[test]
fn dry_run_prints_the_request_and_sends_nothing() {
    let mock = Mock::start(fixture);
    let target = format!("{RID}@2026-10-23");
    let output = mock
        .cmd()
        .args(["subtask", "add", &target, "Warm up", "--dry-run", "--json"])
        .output()
        .unwrap();
    let result = &stdout_json(&output)["result"];
    assert_eq!(result["dry_run"], true);
    assert_eq!(result["tool"], "update_task");
    assert_eq!(result["arguments"]["update_action"], "only_this");
    assert!(
        mock.calls()
            .iter()
            .all(|call| call["tool"] != "update_task")
    );
}

#[test]
fn argument_errors_are_json_when_piped() {
    let mock = Mock::start(fixture);
    for args in [
        vec!["bogus"],
        vec!["day"],
        vec!["list", "--limit", "500"],
        vec!["today", "--json", "--text"],
    ] {
        let output = mock.cmd().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let error: Value = serde_json::from_slice(&output.stderr).expect("stderr is JSON");
        assert_eq!(error["error"]["code"], "USAGE", "{args:?}");
    }
}

#[test]
fn bad_input_is_refused_before_any_call() {
    let mock = Mock::start(fixture);
    let target = format!("{RID}@2026-10-23");
    for args in [
        vec!["add", "  ", "--json"],
        vec!["list", "--from", "+7", "--to", "today", "--json"],
        vec!["done", &target, "--day", "2026-10-24", "--json"],
        vec!["add", "x", "--time", "9:5", "--json"],
        vec!["login", "--json"],
    ] {
        mock.cmd().args(&args).assert().code(2);
    }
    assert!(mock.calls().is_empty());
}

#[test]
fn missing_tasks_are_not_found_with_recovery_steps() {
    let mock = Mock::start(|tool, _| match tool {
        "complete_task" => json!({"isError": true, "text": "Task not found"}),
        _ => json!({"tasks": []}),
    });
    let output = mock
        .cmd()
        .args(["done", "nope", "--json"])
        .output()
        .unwrap();
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "NOT_FOUND");
    assert!(
        error["error"]["next_steps"][0]
            .as_str()
            .unwrap()
            .contains("today --json")
    );
}

#[test]
fn add_with_only_a_time_lands_today_and_timed() {
    let mock = Mock::start(fixture);
    mock.cmd()
        .args([
            "add",
            "Call",
            "--time",
            "09:30",
            "--subtask",
            "Dial",
            "--json",
        ])
        .assert()
        .success();
    let args = mock.call("create_task");
    assert_eq!(args["start_time"], 9.5);
    assert!(args["day"].as_str().is_some_and(|day| day.len() == 10));
    assert!(args.get("is_all_day").is_none());
    assert_eq!(args["subtasks"], json!([{"title": "Dial"}]));
}

#[test]
fn list_filters_map_to_server_arguments() {
    let mock = Mock::start(fixture);
    mock.cmd()
        .args([
            "list",
            "--from",
            "2026-10-01",
            "--open",
            "--search",
            "run",
            "--limit",
            "5",
        ])
        .assert()
        .success();
    let args = mock.call("list_tasks");
    assert_eq!(args["day_from"], "2026-10-01");
    assert_eq!(args["is_completed"], false);
    assert_eq!(args["title_contains"], "run");
    assert_eq!(args["limit"], 5);
}

#[test]
fn piped_account_is_json_and_text_flag_prints_a_timeline() {
    let mock = Mock::start(fixture);
    let output = mock.cmd().arg("account").output().unwrap();
    assert_eq!(stdout_json(&output)["ok"], true);
    mock.cmd()
        .args(["today", "--text"])
        .assert()
        .success()
        .stdout(predicate::str::contains("All day"))
        .stdout(predicate::str::contains("[ ] Run  (0/1)  repeats"))
        .stdout(predicate::str::contains("    [ ] Stretch"));
}

#[test]
fn subtask_delete_needs_yes_and_touches_nothing() {
    let mock = Mock::start(fixture);
    mock.cmd()
        .args(["subtask", "delete", "task-1", "1", "--json"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("pass --yes"));
    assert!(mock.calls().is_empty());
}

#[test]
fn unknown_subtask_number_is_a_usage_error() {
    let mock = Mock::start(fixture);
    mock.cmd()
        .args(["subtask", "done", "task-1", "9", "--json"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("out of range"));
}

#[test]
fn missing_arguments_are_named_with_a_runnable_help_step() {
    let mock = Mock::start(fixture);
    let output = mock
        .cmd()
        .args(["subtask", "done", "task-1", "--json"])
        .output()
        .unwrap();
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("<SUB>"),
        "{error}"
    );
    assert_eq!(
        error["error"]["next_steps"],
        json!(["structured subtask done --help"])
    );
}

#[test]
fn a_recurring_days_plain_id_points_at_its_ref() {
    let mock = Mock::start(fixture);
    let output = mock
        .cmd()
        .args(["subtask", "list", "occ-1", "--json"])
        .output()
        .unwrap();
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "NOT_FOUND");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains(&format!("{RID}@2026-10-23"))
    );
}

#[test]
fn previews_and_no_op_ticks_send_nothing() {
    let mock = Mock::start(fixture);
    mock.cmd()
        .args(["delete", "task-1", "--dry-run", "--json"])
        .assert()
        .success();
    let target = format!("{RID}@2026-10-23");
    mock.cmd()
        .args(["subtask", "undo", &target, "1", "--json"])
        .assert()
        .success();
    assert!(
        mock.calls()
            .iter()
            .all(|call| call["tool"] != "update_task" && call["tool"] != "delete_task")
    );
}
