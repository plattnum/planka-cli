//! E2E tests: top-level command aliases produce identical output to the
//! canonical resource/action form. Uses wiremock so tests run without a
//! live Planka server.

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn plnk() -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd
}

fn plnk_with_server(server_uri: &str) -> Command {
    let mut cmd = plnk();
    cmd.env("PLANKA_SERVER", server_uri);
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd
}

fn run_json(server_uri: &str, args: &[&str]) -> Value {
    let output = plnk_with_server(server_uri)
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

// ─── Alias parity: boards ───────────────────────────────────────────

#[tokio::test]
async fn boards_alias_json_matches_canonical() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/proj-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"proj-1","name":"Test","createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {
                "boards": [{"id":"board-1","projectId":"proj-1","name":"Sprint","position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}],
                "projectManagers": []
            }
        })))
        .mount(&server)
        .await;

    let canonical = run_json(
        &server.uri(),
        &["board", "list", "--project", "proj-1", "--output", "json"],
    );
    let alias = run_json(
        &server.uri(),
        &["boards", "--project", "proj-1", "--output", "json"],
    );

    assert_eq!(canonical, alias);
}

// ─── Alias parity: lists ────────────────────────────────────────────

#[tokio::test]
async fn lists_alias_json_matches_canonical() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"board-1","projectId":"proj-1","name":"Sprint","position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {
                "lists": [{"id":"list-1","boardId":"board-1","name":"Todo","position":65536.0,"type":"active","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}],
                "cards": [],
                "tasks": [],
                "labels": [],
                "cardLabels": [],
                "cardMemberships": [],
                "boardMemberships": [],
                "users": []
            }
        })))
        .mount(&server)
        .await;

    let canonical = run_json(
        &server.uri(),
        &["list", "list", "--board", "board-1", "--output", "json"],
    );
    let alias = run_json(
        &server.uri(),
        &["lists", "--board", "board-1", "--output", "json"],
    );

    assert_eq!(canonical, alias);
}

// ─── Alias parity: cards ────────────────────────────────────────────

#[tokio::test]
async fn cards_alias_json_matches_canonical() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/lists/list-1/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [{"id":"card-1","listId":"list-1","boardId":"board-1","name":"Fix bug","description":null,"position":65536.0,"isSubscribed":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}],
            "included": {
                "cardLabels": [],
                "cardMemberships": []
            }
        })))
        .mount(&server)
        .await;

    let canonical = run_json(
        &server.uri(),
        &["card", "list", "--list", "list-1", "--output", "json"],
    );
    let alias = run_json(
        &server.uri(),
        &["cards", "--list", "list-1", "--output", "json"],
    );

    assert_eq!(canonical, alias);
}

// ─── Alias parity: cards (--board scope) ────────────────────────────

#[tokio::test]
async fn cards_alias_board_scope_json_matches_canonical() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"board-1","projectId":"proj-1","name":"Sprint","position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {
                "lists": [{"id":"list-1","boardId":"board-1","name":"Todo","position":65536.0,"type":"active","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}],
                "cards": [
                    {"id":"card-1","listId":"list-1","boardId":"board-1","name":"Fix bug","description":null,"position":65536.0,"isSubscribed":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
                    {"id":"card-2","listId":"list-1","boardId":"board-1","name":"Add feature","description":null,"position":131_072.0,"isSubscribed":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
                ],
                "tasks": [],
                "labels": [],
                "cardLabels": [],
                "cardMemberships": [],
                "boardMemberships": [],
                "users": []
            }
        })))
        .mount(&server)
        .await;

    let canonical = run_json(
        &server.uri(),
        &["card", "list", "--board", "board-1", "--output", "json"],
    );
    let alias = run_json(
        &server.uri(),
        &["cards", "--board", "board-1", "--output", "json"],
    );

    assert_eq!(canonical, alias);
}

// ─── Alias parity: tasks ────────────────────────────────────────────

#[tokio::test]
async fn tasks_alias_json_matches_canonical() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"card-1","listId":"list-1","boardId":"board-1","name":"Fix bug","description":null,"position":65536.0,"isSubscribed":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {
                "tasks": [{"id":"task-1","taskListId":"tl-1","name":"Write tests","isCompleted":false,"position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}],
                "taskLists": [{"id":"tl-1","cardId":"card-1","name":"Tasks","position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}],
                "cardLabels": [],
                "cardMemberships": [],
                "attachments": []
            }
        })))
        .mount(&server)
        .await;

    let canonical = run_json(
        &server.uri(),
        &["task", "list", "--card", "card-1", "--output", "json"],
    );
    let alias = run_json(
        &server.uri(),
        &["tasks", "--card", "card-1", "--output", "json"],
    );

    assert_eq!(canonical, alias);
}

// ─── Alias parity: comments ─────────────────────────────────────────

#[tokio::test]
async fn comments_alias_json_matches_canonical() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [{"id":"comment-1","cardId":"card-1","userId":"user-1","text":"Starting work","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}]
        })))
        .mount(&server)
        .await;

    let canonical = run_json(
        &server.uri(),
        &["comment", "list", "--card", "card-1", "--output", "json"],
    );
    let alias = run_json(
        &server.uri(),
        &["comments", "--card", "card-1", "--output", "json"],
    );

    assert_eq!(canonical, alias);
}

// ─── Alias parity: labels ───────────────────────────────────────────

#[tokio::test]
async fn labels_alias_json_matches_canonical() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"board-1","projectId":"proj-1","name":"Sprint","position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {
                "lists": [],
                "cards": [],
                "tasks": [],
                "labels": [{"id":"label-1","boardId":"board-1","name":"bug","color":"berry-red","position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}],
                "cardLabels": [],
                "cardMemberships": [],
                "boardMemberships": [],
                "users": []
            }
        })))
        .mount(&server)
        .await;

    let canonical = run_json(
        &server.uri(),
        &["label", "list", "--board", "board-1", "--output", "json"],
    );
    let alias = run_json(
        &server.uri(),
        &["labels", "--board", "board-1", "--output", "json"],
    );

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
    // Validation happens before network, so no server needed.
    plnk()
        .env("PLANKA_SERVER", "http://127.0.0.1:1")
        .env("PLANKA_TOKEN", "test-api-key")
        .args(["boards"])
        .assert()
        .failure()
        .code(2);
}
