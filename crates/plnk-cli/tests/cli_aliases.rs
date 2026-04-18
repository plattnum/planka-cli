use assert_cmd::Command;
use serde_json::Value;

const TEST_SERVER: &str = "http://storm-front:3002";
const TEST_TOKEN: &str = "tNub244N_MBnBqhLH7PE2fjwQD9w2w69t6f3uCrPM";
const TEST_PROJECT: &str = "1753611015817266606";

fn plnk() -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd
}

fn plnk_authed() -> Command {
    let mut cmd = plnk();
    cmd.env("PLANKA_SERVER", TEST_SERVER);
    cmd.env("PLANKA_TOKEN", TEST_TOKEN);
    cmd
}

fn run_json(args: &[&str]) -> Value {
    let output = plnk_authed()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn first_id(data: &Value) -> String {
    data["data"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
}

fn current_board_id() -> String {
    let boards = run_json(&[
        "board",
        "list",
        "--project",
        TEST_PROJECT,
        "--output",
        "json",
    ]);
    first_id(&boards)
}

fn current_list_id() -> String {
    let board_id = current_board_id();
    let lists = run_json(&["list", "list", "--board", &board_id, "--output", "json"]);
    first_id(&lists)
}

fn current_card_id() -> String {
    let boards = run_json(&[
        "board",
        "list",
        "--project",
        TEST_PROJECT,
        "--output",
        "json",
    ]);
    for board in boards["data"].as_array().unwrap() {
        let board_id = board["id"].as_str().unwrap();
        let snapshot = run_json(&["board", "snapshot", board_id, "--output", "json"]);
        if let Some(card_id) = snapshot["data"]["included"]["cards"]
            .as_array()
            .and_then(|cards| cards.first())
            .and_then(|card| card.get("id"))
            .and_then(Value::as_str)
        {
            return card_id.to_string();
        }
    }
    panic!("expected at least one card in project {TEST_PROJECT}");
}

// ─── Alias parity: boards ───────────────────────────────────────────

#[test]
fn boards_alias_json_matches_canonical() {
    let canonical = plnk_authed()
        .args([
            "board",
            "list",
            "--project",
            TEST_PROJECT,
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let alias = plnk_authed()
        .args(["boards", "--project", TEST_PROJECT, "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let canonical_json: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
    let alias_json: serde_json::Value = serde_json::from_slice(&alias).unwrap();
    assert_eq!(canonical_json, alias_json);
}

// ─── Alias parity: lists ────────────────────────────────────────────

#[test]
fn lists_alias_json_matches_canonical() {
    let board_id = current_board_id();

    let canonical = run_json(&["list", "list", "--board", &board_id, "--output", "json"]);
    let alias = run_json(&["lists", "--board", &board_id, "--output", "json"]);

    assert_eq!(canonical, alias);
}

// ─── Alias parity: cards ────────────────────────────────────────────

#[test]
fn cards_alias_json_matches_canonical() {
    let list_id = current_list_id();

    let canonical = run_json(&["card", "list", "--list", &list_id, "--output", "json"]);
    let alias = run_json(&["cards", "--list", &list_id, "--output", "json"]);

    assert_eq!(canonical, alias);
}

// ─── Alias parity: tasks ────────────────────────────────────────────

#[test]
fn tasks_alias_json_matches_canonical() {
    let card_id = current_card_id();

    let canonical = run_json(&["task", "list", "--card", &card_id, "--output", "json"]);
    let alias = run_json(&["tasks", "--card", &card_id, "--output", "json"]);

    assert_eq!(canonical, alias);
}

// ─── Alias parity: comments ─────────────────────────────────────────

#[test]
fn comments_alias_json_matches_canonical() {
    let card_id = current_card_id();

    let canonical = run_json(&["comment", "list", "--card", &card_id, "--output", "json"]);
    let alias = run_json(&["comments", "--card", &card_id, "--output", "json"]);

    assert_eq!(canonical, alias);
}

// ─── Alias parity: labels ───────────────────────────────────────────

#[test]
fn labels_alias_json_matches_canonical() {
    let board_id = current_board_id();

    let canonical = run_json(&["label", "list", "--board", &board_id, "--output", "json"]);
    let alias = run_json(&["labels", "--board", &board_id, "--output", "json"]);

    assert_eq!(canonical, alias);
}

// ─── Aliases hidden from help ───────────────────────────────────────

#[test]
fn aliases_hidden_from_help() {
    // Alias commands (boards, lists, cards, tasks, comments, labels) must not
    // appear as command entries. We use regex to match the command listing format
    // "  <name> " at the start of a line, since descriptions like "Manage boards"
    // contain these words as substrings.
    let output = plnk()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let help = String::from_utf8(output).unwrap();

    // In clap's help, commands are listed as "  <name>  <description>"
    // Check that none of the alias names appear as command entries
    for alias in &["boards", "lists", "cards", "tasks", "comments", "labels"] {
        let pattern = format!("  {alias} ");
        assert!(
            !help.contains(&pattern),
            "alias '{alias}' should be hidden from help but found: {pattern}"
        );
    }
}

// ─── Alias missing required flag ────────────────────────────────────

#[test]
fn boards_alias_missing_project_exits_2() {
    plnk_authed().args(["boards"]).assert().failure().code(2);
}
