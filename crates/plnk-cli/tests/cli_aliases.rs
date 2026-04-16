use assert_cmd::Command;
use predicates::prelude::*;

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
fn lists_alias_works() {
    // m-4 board has known lists
    let board_id = "1753741391671854585";
    plnk_authed()
        .args(["lists", "--board", board_id, "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"));
}

// ─── Alias parity: cards ────────────────────────────────────────────

#[test]
fn cards_alias_works() {
    // In Progress list on m-4 board
    let list_id = "1753741392158393855";
    plnk_authed()
        .args(["cards", "--list", list_id, "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"));
}

// ─── Alias parity: tasks ────────────────────────────────────────────

#[test]
fn tasks_alias_works() {
    // PLNK-018 card
    let card_id = "1753741396461749796";
    plnk_authed()
        .args(["tasks", "--card", card_id, "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"));
}

// ─── Alias parity: comments ─────────────────────────────────────────

#[test]
fn comments_alias_works() {
    let card_id = "1753741396461749796";
    plnk_authed()
        .args(["comments", "--card", card_id, "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"));
}

// ─── Alias parity: labels ───────────────────────────────────────────

#[test]
fn labels_alias_works() {
    let board_id = "1753741391671854585";
    plnk_authed()
        .args(["labels", "--board", board_id, "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"));
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
