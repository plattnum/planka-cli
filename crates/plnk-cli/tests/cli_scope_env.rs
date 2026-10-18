//! E2E tests: `--project` / `--board` / `--list` fall back to
//! `PLNK_PROJECT` / `PLNK_BOARD` / `PLNK_LIST`.
//!
//! Each test mounts a mock only on the endpoint the selected scope must
//! reach, so a wrong scope fails with a 404 rather than passing quietly.

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn plnk(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env("PLANKA_SERVER", server_uri);
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd.env_remove("PLNK_PROJECT");
    cmd.env_remove("PLNK_BOARD");
    cmd.env_remove("PLNK_LIST");
    cmd
}

fn project_snapshot() -> serde_json::Value {
    serde_json::json!({
        "item": {
            "id": "proj-env",
            "name": "Env Project",
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": null
        },
        "included": {"boards": [], "projectManagers": []}
    })
}

fn board_snapshot() -> serde_json::Value {
    serde_json::json!({
        "item": {
            "id": "board-env",
            "projectId": "proj-env",
            "name": "Env Board",
            "position": 65536.0,
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": null
        },
        "included": {
            "lists": [],
            "cards": [],
            "tasks": [],
            "labels": [],
            "cardLabels": [],
            "cardMemberships": [],
            "boardMemberships": [],
            "users": []
        }
    })
}

fn empty_card_list() -> serde_json::Value {
    serde_json::json!({
        "items": [],
        "included": {"cardLabels": [], "cardMemberships": []}
    })
}

// ─── Single-scope arguments (clap `env`) ────────────────────────────

#[tokio::test]
async fn board_list_reads_plnk_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/proj-env"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_snapshot()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_PROJECT", "proj-env")
        .args(["board", "list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\": true"));
}

#[tokio::test]
async fn list_list_reads_plnk_board() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/boards/board-env"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_BOARD", "board-env")
        .args(["list", "list", "--output", "json"])
        .assert()
        .success();
}

#[tokio::test]
async fn card_create_reads_plnk_list() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/lists/list-env/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "card-1",
                "listId": "list-env",
                "boardId": "board-env",
                "name": "A card",
                "description": null,
                "position": 65536.0,
                "isSubscribed": false,
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": null
            }
        })))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_LIST", "list-env")
        .args(["card", "create", "--title", "A card", "--output", "json"])
        .assert()
        .success();
}

#[tokio::test]
async fn explicit_flag_overrides_env_default() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/proj-flag"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_snapshot()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_PROJECT", "proj-env")
        .args(["board", "list", "--project", "proj-flag"])
        .assert()
        .success();
}

// ─── Grouped scope arguments (post-parse fallback) ──────────────────

#[tokio::test]
async fn card_list_reads_plnk_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/lists/list-env/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_card_list()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_LIST", "list-env")
        .args(["card", "list", "--output", "json"])
        .assert()
        .success();
}

#[tokio::test]
async fn card_list_reads_plnk_board() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/boards/board-env"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_BOARD", "board-env")
        .args(["card", "list", "--output", "json"])
        .assert()
        .success();
}

#[tokio::test]
async fn card_find_reads_plnk_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/proj-env"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_snapshot()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_PROJECT", "proj-env")
        .args(["card", "find", "--title", "x", "--output", "json"])
        .assert()
        .success();
}

/// An explicit narrower scope wins over a broader env default instead of
/// conflicting with it. With a plain `clap` `env` attribute this call exits 2.
#[tokio::test]
async fn explicit_board_suppresses_plnk_project_env() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/boards/board-flag"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_PROJECT", "proj-env")
        .args(["card", "find", "--board", "board-flag", "--title", "x"])
        .assert()
        .success();
}

/// With several env defaults set, the narrowest scope wins.
#[tokio::test]
async fn narrower_env_default_wins() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/lists/list-env/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_card_list()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_LIST", "list-env")
        .env("PLNK_BOARD", "board-env")
        .env("PLNK_PROJECT", "proj-env")
        .args(["card", "list", "--output", "json"])
        .assert()
        .success();
}

#[tokio::test]
async fn field_group_list_reads_plnk_board() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/boards/board-env"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot()))
        .mount(&server)
        .await;

    plnk(&server.uri())
        .env("PLNK_BOARD", "board-env")
        .args(["field-group", "list", "--output", "json"])
        .assert()
        .success();
}

// ─── Existing validation still applies ──────────────────────────────

#[test]
fn card_list_without_scope_still_errors() {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env("PLANKA_SERVER", "http://127.0.0.1:9");
    cmd.env("PLANKA_TOKEN", "test-api-key");
    // `PLNK_PROJECT` is not a valid scope for `card list`.
    cmd.env("PLNK_PROJECT", "proj-env");
    cmd.env_remove("PLNK_BOARD");
    cmd.env_remove("PLNK_LIST");

    cmd.args(["card", "list"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Missing required option"));
}

#[test]
fn board_list_without_project_still_errors() {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env("PLANKA_SERVER", "http://127.0.0.1:9");
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd.env_remove("PLNK_PROJECT");

    cmd.args(["board", "list"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("required"));
}

// ─── Empty env values are rejected ──────────────────────────────────

/// An empty value must not be read as "unset". Each variable is rejected on
/// its own, even when the command in play does not accept that scope at all.
#[test]
fn empty_scope_env_is_rejected() {
    for name in ["PLNK_PROJECT", "PLNK_BOARD", "PLNK_LIST"] {
        let mut cmd = Command::cargo_bin("plnk").unwrap();
        cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
        cmd.env("PLANKA_SERVER", "http://127.0.0.1:9");
        cmd.env("PLANKA_TOKEN", "test-api-key");
        cmd.env_remove("PLNK_PROJECT");
        cmd.env_remove("PLNK_BOARD");
        cmd.env_remove("PLNK_LIST");
        cmd.env(name, "");

        cmd.args(["board", "list"])
            .assert()
            .failure()
            .code(2)
            .stderr(predicate::str::contains(name))
            .stderr(predicate::str::contains("must not be empty"));
    }
}

/// Whitespace-only values are as unusable as empty ones.
#[test]
fn whitespace_scope_env_is_rejected() {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env("PLANKA_SERVER", "http://127.0.0.1:9");
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd.env_remove("PLNK_PROJECT");
    cmd.env_remove("PLNK_BOARD");
    cmd.env("PLNK_LIST", "   ");

    cmd.args(["card", "list"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("PLNK_LIST"))
        .stderr(predicate::str::contains("must not be empty"));
}

/// An empty env var beats an explicit flag: the environment is validated
/// before the command runs, so a stale export cannot be papered over.
#[test]
fn empty_scope_env_beats_explicit_flag() {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env("PLANKA_SERVER", "http://127.0.0.1:9");
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd.env("PLNK_PROJECT", "");
    cmd.env_remove("PLNK_BOARD");
    cmd.env_remove("PLNK_LIST");

    cmd.args(["board", "list", "--project", "proj-flag"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("PLNK_PROJECT"));
}
