//! E2E tests: output modes (JSON envelope, quiet, verbose), card find.

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn plnk_with_server(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env("PLANKA_SERVER", server_uri);
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd
}

// ─── JSON envelope for every resource list command ──────────────────

#[tokio::test]
async fn json_envelope_project_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"items": [{"id":"1","name":"P","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}]})),
        )
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["project", "list", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    assert!(json["data"].is_array());
    assert!(json["meta"]["count"].is_number());
}

#[tokio::test]
async fn json_envelope_user_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"items": [{"id":"1","name":"Admin","username":"admin","email":"a@b.com","role":"admin","isDeactivated":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}]})),
        )
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["user", "list", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["meta"]["count"], 1);
}

// ─── Empty collection returns count: 0 ─────────────────────────────

#[tokio::test]
async fn empty_collection_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"items": []})))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["project", "list", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"], serde_json::json!([]));
    assert_eq!(json["meta"]["count"], 0);
}

// ─── --quiet suppresses all output ──────────────────────────────────

#[tokio::test]
async fn quiet_suppresses_output() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"items": [{"id":"1","name":"P","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}]})),
        )
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["project", "list", "--quiet"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

// ─── -vv produces debug logs on stderr without corrupting stdout ────

#[tokio::test]
async fn verbose_logs_to_stderr_not_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"items": [{"id":"1","name":"P","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}]})),
        )
        .mount(&server)
        .await;

    let cmd_output = plnk_with_server(&server.uri())
        .args(["project", "list", "--output", "json", "-vv"])
        .assert()
        .success()
        .get_output()
        .clone();

    // stderr should have debug logs (e.g., GET request)
    let stderr = String::from_utf8(cmd_output.stderr).unwrap();
    assert!(
        stderr.contains("GET"),
        "stderr should contain debug logs, got: {stderr}"
    );

    // stdout should still be valid JSON
    let json: serde_json::Value = serde_json::from_slice(&cmd_output.stdout).unwrap();
    assert_eq!(json["success"], true);
}

// ─── Card find returns collection (not error) ───────────────────────

#[tokio::test]
async fn card_find_multiple_results() {
    let server = MockServer::start().await;

    // card find --list uses GET /api/lists/{id}/cards
    Mock::given(method("GET"))
        .and(path("/api/lists/list-1/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [
                {
                    "id": "card-1",
                    "listId": "list-1",
                    "boardId": "board-1",
                    "name": "Fix auth",
                    "description": null,
                    "position": 65536.0,
                    "isSubscribed": false,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                },
                {
                    "id": "card-2",
                    "listId": "list-1",
                    "boardId": "board-1",
                    "name": "Fix auth race",
                    "description": null,
                    "position": 131_072.0,
                    "isSubscribed": false,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                },
                {
                    "id": "card-3",
                    "listId": "list-1",
                    "boardId": "board-1",
                    "name": "Unrelated",
                    "description": null,
                    "position": 196_608.0,
                    "isSubscribed": false,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ],
            "included": {
                "cardLabels": [],
                "cardMemberships": []
            }
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card", "find", "--list", "list-1", "--title", "Fix auth", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    // "Fix auth" exact match → tier 1 → returns "Fix auth" only (not "Fix auth race")
    // Actually: tier 1 exact case-sensitive finds "Fix auth", stops there
    assert_eq!(json["meta"]["count"], 1);
    assert_eq!(json["data"][0]["id"], "card-1");
}

#[tokio::test]
async fn card_find_substring_matches() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/lists/list-1/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [
                {
                    "id": "card-1",
                    "listId": "list-1",
                    "boardId": "board-1",
                    "name": "Fix auth bug",
                    "description": null,
                    "position": 65536.0,
                    "isSubscribed": false,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                },
                {
                    "id": "card-2",
                    "listId": "list-1",
                    "boardId": "board-1",
                    "name": "Fix auth race",
                    "description": null,
                    "position": 131_072.0,
                    "isSubscribed": false,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ],
            "included": {
                "cardLabels": [],
                "cardMemberships": []
            }
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card", "find", "--list", "list-1", "--title", "auth", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    // "auth" → no exact match → no case-insensitive exact → substring matches both
    assert_eq!(json["meta"]["count"], 2);
}

// ─── Board list (scoped) → card create → task create → comment create ─

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn board_card_task_comment_flow() {
    let server = MockServer::start().await;

    // Step 1: Board list (from project snapshot)
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

    let output = plnk_with_server(&server.uri())
        .args(["board", "list", "--project", "proj-1", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["meta"]["count"], 1);
    assert_eq!(json["data"][0]["name"], "Sprint");

    // Step 2: Card create
    Mock::given(method("POST"))
        .and(path("/api/lists/list-1/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"card-new","listId":"list-1","boardId":"board-1","name":"Test Card","description":null,"position":65536.0,"isSubscribed":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card",
            "create",
            "--list",
            "list-1",
            "--title",
            "Test Card",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["name"], "Test Card");

    // Step 3: Task create (needs card snapshot first)
    Mock::given(method("GET"))
        .and(path("/api/cards/card-new"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"card-new","listId":"list-1","boardId":"board-1","name":"Test Card","description":null,"position":65536.0,"isSubscribed":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {
                "tasks": [],
                "taskLists": [{"id":"tl-1","cardId":"card-new","name":"Tasks","position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}],
                "cardLabels": [],
                "cardMemberships": [],
                "attachments": []
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/task-lists/tl-1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"task-1","taskListId":"tl-1","name":"Write tests","isCompleted":false,"position":65536.0,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "task",
            "create",
            "--card",
            "card-new",
            "--title",
            "Write tests",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["name"], "Write tests");

    // Step 4: Comment create
    Mock::given(method("POST"))
        .and(path("/api/cards/card-new/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"comment-1","cardId":"card-new","userId":"user-1","text":"Starting work","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "comment",
            "create",
            "--card",
            "card-new",
            "--text",
            "Starting work",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["text"], "Starting work");
}

// ─── Markdown output works ──────────────────────────────────────────

#[tokio::test]
async fn markdown_output_project() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"42","name":"Platform","createdAt":"2026-04-14T12:00:00Z","updatedAt":null},
            "included": {"boards": [], "projectManagers": []}
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["project", "get", "42", "--output", "markdown"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Platform"))
        .stdout(predicate::str::contains("**"));
}
