use assert_cmd::Command;
use predicates::prelude::*;

fn plnk() -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd
}

// ─── Machine-readable help: action level ────────────────────────────

#[test]
fn machine_help_card_create() {
    let output = plnk()
        .args(["card", "create", "--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["resource"], "card");
    assert_eq!(json["action"], "create");
    assert!(json["summary"].is_string());
    assert!(!json["summary"].as_str().unwrap().is_empty());

    // Options present with correct types
    let opts = &json["options"];
    assert_eq!(opts["--list"]["type"], "string");
    assert_eq!(opts["--list"]["required"], true);
    assert_eq!(opts["--title"]["type"], "string");
    assert_eq!(opts["--title"]["required"], true);
    assert_eq!(opts["--description"]["type"], "text");
    assert_eq!(opts["--description"]["required"], false);
    assert_eq!(opts["--position"]["type"], "enum(top|bottom|int)");

    // Examples present
    assert!(json["examples"].is_array());
    assert!(!json["examples"].as_array().unwrap().is_empty());

    // Global flags included, including transport tuning knobs
    assert_eq!(opts["--server"]["type"], "string");
    assert_eq!(opts["--token"]["type"], "string");
    assert_eq!(opts["--output"]["type"], "string");
    assert_eq!(opts["--http-max-in-flight"]["type"], "integer");
    assert_eq!(opts["--retry-attempts"]["type"], "integer");
    assert_eq!(opts["--no-retry"]["type"], "flag");
}

#[test]
fn machine_help_board_list() {
    let output = plnk()
        .args(["board", "list", "--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["resource"], "board");
    assert_eq!(json["action"], "list");
    assert_eq!(json["options"]["--project"]["required"], true);
}

#[test]
fn machine_help_card_get_many() {
    let output = plnk()
        .args(["card", "get-many", "--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let opts = &json["options"];
    assert_eq!(json["resource"], "card");
    assert_eq!(json["action"], "get-many");
    assert_eq!(opts["--id"]["type"], "string");
    assert_eq!(opts["--id"]["required"], true);
    assert_eq!(opts["--concurrency"]["type"], "integer");
    assert_eq!(opts["--allow-missing"]["type"], "flag");
    assert!(
        json["examples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|example| example.as_str().unwrap().contains("get-many"))
    );
}

#[test]
fn machine_help_card_list_includes_label_option_and_examples() {
    let output = plnk()
        .args(["card", "list", "--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let opts = &json["options"];
    assert_eq!(opts["--label"]["type"], "string");
    assert_eq!(opts["--label"]["required"], false);
    assert!(
        opts["--label"]["description"]
            .as_str()
            .unwrap()
            .contains("use an ID to avoid ambiguity")
    );
    assert!(
        json["examples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|example| example.as_str().unwrap().contains("--label"))
    );
}

#[test]
fn machine_help_card_find_includes_label_option_and_examples() {
    let output = plnk()
        .args(["card", "find", "--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let opts = &json["options"];
    assert_eq!(opts["--label"]["type"], "string");
    assert_eq!(opts["--label"]["required"], false);
    assert!(
        json["examples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|example| example.as_str().unwrap().contains("--label"))
    );
}

#[test]
fn machine_help_task_complete() {
    let output = plnk()
        .args(["task", "complete", "--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["resource"], "task");
    assert_eq!(json["action"], "complete");
    // Positional arg "id"
    let args = json["args"].as_array().unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0]["name"], "id");
    assert_eq!(args[0]["required"], true);
}

// ─── Machine-readable help: resource level ──────────────────────────

#[test]
fn machine_help_resource_level() {
    let output = plnk()
        .args(["card", "--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["resource"], "card");
    assert_eq!(json["action"], "");

    // Lists available actions
    let opts = &json["options"];
    assert!(opts.get("list").is_some());
    assert!(opts.get("get").is_some());
    assert!(opts.get("get-many").is_some());
    assert!(opts.get("create").is_some());
    assert!(opts.get("find").is_some());
    assert!(opts.get("move").is_some());
}

// ─── Machine-readable help: top level ───────────────────────────────

#[test]
fn machine_help_top_level() {
    let output = plnk()
        .args(["--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["resource"], "");

    // Lists all visible resources
    let opts = &json["options"];
    assert!(opts.get("auth").is_some());
    assert!(opts.get("card").is_some());
    assert!(opts.get("project").is_some());
    assert!(opts.get("board").is_some());

    // Aliases hidden
    assert!(opts.get("boards").is_none());
    assert!(opts.get("cards").is_none());
}

// ─── Machine-readable help: nested subcommand ───────────────────────

#[test]
fn machine_help_card_label_add() {
    let output = plnk()
        .args(["card", "label", "add", "--help", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["resource"], "card");
    assert_eq!(json["action"], "label add");

    let args = json["args"].as_array().unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0]["name"], "card");
    assert_eq!(args[1]["name"], "label");
}

// ─── Machine help: all resources have valid structure ────────────────

#[test]
fn machine_help_all_resources() {
    let resources = [
        "auth",
        "user",
        "project",
        "board",
        "list",
        "card",
        "task",
        "comment",
        "label",
        "attachment",
        "membership",
    ];

    for resource in &resources {
        let output = plnk()
            .args([resource, "--help", "--output", "json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(
            json["resource"].as_str().unwrap(),
            *resource,
            "resource mismatch for {resource}"
        );
        assert!(
            json["options"].is_object(),
            "options should be an object for {resource}"
        );
    }
}

// ─── Normal help still works with --output table ────────────────────

#[test]
fn normal_help_still_works() {
    // Without --output json, --help should produce normal clap help
    plnk()
        .args(["card", "create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Create a new card"))
        .stdout(predicate::str::contains("--list"))
        .stdout(predicate::str::contains("--title"));
}
