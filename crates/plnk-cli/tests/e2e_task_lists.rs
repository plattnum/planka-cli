//! End-to-end tests for `plnk task-list` and the checklist-aware `plnk task`.
//!
//! The behaviour worth pinning is what happens on a card with more than one
//! checklist: `task create` must refuse to guess, and `--task-list` must
//! resolve a name to exactly one list.

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn plnk_with_server(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env("PLANKA_SERVER", server_uri);
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd
}

fn task_list_json(id: &str, name: &str, position: f64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "cardId": "card-1",
        "name": name,
        "position": position,
        "showOnFrontOfCard": true,
        "hideCompletedTasks": false,
        "createdAt": "2026-09-12T00:00:00Z",
        "updatedAt": null
    })
}

fn card_snapshot(task_lists: &[serde_json::Value]) -> serde_json::Value {
    serde_json::json!({
        "item": {
            "id": "card-1", "listId": "list-1", "boardId": "board-1", "name": "A card",
            "description": null, "position": 65536.0, "dueDate": null,
            "isDueCompleted": null, "isClosed": false, "isSubscribed": false,
            "creatorUserId": null, "createdAt": "2026-09-12T00:00:00Z", "updatedAt": null
        },
        "included": {
            "taskLists": task_lists,
            "tasks": [],
            "cardLabels": [],
            "cardMemberships": [],
            "attachments": []
        }
    })
}

async fn mount_card(server: &MockServer, task_lists: Vec<serde_json::Value>) {
    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(card_snapshot(&task_lists)))
        .mount(server)
        .await;
}

// ─── plnk task-list ─────────────────────────────────────────────────

#[tokio::test]
async fn task_list_create_omits_flags_when_not_given() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/cards/card-1/task-lists"))
        .and(body_json(
            serde_json::json!({"name": "QA", "position": 65536.0}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"item": task_list_json("tl-9", "QA", 65536.0)})),
        )
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "task-list",
            "create",
            "--card",
            "card-1",
            "--name",
            "QA",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"showOnFrontOfCard\": true"));
}

#[tokio::test]
async fn task_list_create_sends_explicit_flags() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/cards/card-1/task-lists"))
        .and(body_json(serde_json::json!({
            "name": "QA", "position": 65536.0,
            "showOnFrontOfCard": false, "hideCompletedTasks": true
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"item": task_list_json("tl-9", "QA", 65536.0)})),
        )
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "task-list",
            "create",
            "--card",
            "card-1",
            "--name",
            "QA",
            "--show-on-front",
            "false",
            "--hide-completed",
            "true",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn task_list_create_rejects_empty_name_without_request() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/cards/card-1/task-lists"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["task-list", "create", "--card", "card-1", "--name", ""])
        .assert()
        .code(2);
}

#[tokio::test]
async fn task_list_update_with_no_fields_exits_2() {
    let server = MockServer::start().await;

    plnk_with_server(&server.uri())
        .args(["task-list", "update", "tl-1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("At least one field"));
}

#[tokio::test]
async fn task_list_move_patches_position() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/task-lists/tl-1"))
        .and(body_json(serde_json::json!({"position": 0.0})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"item": task_list_json("tl-1", "QA", 0.0)})),
        )
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["task-list", "move", "tl-1", "--to-position", "top"])
        .assert()
        .success();
}

#[tokio::test]
async fn task_list_get_renders_item() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/task-lists/tl-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": task_list_json("tl-1", "QA", 65536.0),
            "included": {"tasks": []}
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["task-list", "get", "tl-1", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"QA\""));
}

#[tokio::test]
async fn task_list_find_matches_case_insensitively() {
    let server = MockServer::start().await;
    mount_card(
        &server,
        vec![
            task_list_json("tl-a", "Design", 65536.0),
            task_list_json("tl-b", "QA", 131_072.0),
        ],
    )
    .await;

    plnk_with_server(&server.uri())
        .args([
            "task-list",
            "find",
            "--card",
            "card-1",
            "--name",
            "qa",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": \"tl-b\""))
        .stdout(predicate::str::contains("tl-a").not());
}

#[tokio::test]
async fn task_list_delete_with_yes_skips_prompt() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/task-lists/tl-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": task_list_json("tl-1", "QA", 65536.0)
        })))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["task-list", "delete", "tl-1", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted"));
}

// ─── plnk task, checklist-aware ─────────────────────────────────────

fn task_item(id: &str, task_list_id: &str) -> serde_json::Value {
    serde_json::json!({"item": {
        "id": id, "taskListId": task_list_id, "name": "X", "isCompleted": false,
        "position": 65536.0, "linkedCardId": null, "assigneeUserId": null,
        "createdAt": "2026-09-12T00:00:00Z", "updatedAt": null
    }})
}

#[tokio::test]
async fn create_on_card_with_two_lists_and_no_flag_exits_2_and_posts_nothing() {
    let server = MockServer::start().await;
    mount_card(
        &server,
        vec![
            task_list_json("tl-a", "Design", 65536.0),
            task_list_json("tl-b", "QA", 131_072.0),
        ],
    )
    .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["task", "create", "--card", "card-1", "--title", "X"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--task-list"))
        .stderr(predicate::str::contains("Design (tl-a)"))
        .stderr(predicate::str::contains("QA (tl-b)"));
}

#[tokio::test]
async fn create_with_task_list_name_posts_to_resolved_list() {
    let server = MockServer::start().await;
    mount_card(
        &server,
        vec![
            task_list_json("tl-a", "Design", 65536.0),
            task_list_json("tl-b", "QA", 131_072.0),
        ],
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/task-lists/tl-b/tasks"))
        .and(body_json(
            serde_json::json!({"name": "X", "position": 65536.0}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_item("task-1", "tl-b")))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "task",
            "create",
            "--card",
            "card-1",
            "--title",
            "X",
            "--task-list",
            "qa",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"taskListId\": \"tl-b\""));
}

#[tokio::test]
async fn create_with_task_list_id_posts_without_resolution() {
    let server = MockServer::start().await;
    mount_card(&server, vec![task_list_json("tl-a", "Design", 65536.0)]).await;
    Mock::given(method("POST"))
        .and(path("/api/task-lists/tl-a/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_item("task-1", "tl-a")))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "task",
            "create",
            "--card",
            "card-1",
            "--title",
            "X",
            "--task-list",
            "tl-a",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn create_on_card_with_no_lists_auto_creates_tasks() {
    let server = MockServer::start().await;
    mount_card(&server, vec![]).await;
    Mock::given(method("POST"))
        .and(path("/api/cards/card-1/task-lists"))
        .and(body_json(
            serde_json::json!({"name": "Tasks", "position": 65536.0}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"item": task_list_json("tl-new", "Tasks", 65536.0)}),
            ),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/task-lists/tl-new/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_item("task-1", "tl-new")))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["task", "create", "--card", "card-1", "--title", "X"])
        .assert()
        .success();
}

#[tokio::test]
async fn create_on_card_with_one_list_uses_it() {
    let server = MockServer::start().await;
    mount_card(&server, vec![task_list_json("tl-only", "Tasks", 65536.0)]).await;
    Mock::given(method("POST"))
        .and(path("/api/task-lists/tl-only/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_item("task-1", "tl-only")))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["task", "create", "--card", "card-1", "--title", "X"])
        .assert()
        .success();
}

/// "QA" and "qa" both survive the case-insensitive tier for the query "Qa";
/// an exact "qa" would have won tier one on its own.
#[tokio::test]
async fn ambiguous_task_list_name_exits_2_and_names_candidates() {
    let server = MockServer::start().await;
    mount_card(
        &server,
        vec![
            task_list_json("tl-a", "QA", 65536.0),
            task_list_json("tl-b", "qa", 131_072.0),
        ],
    )
    .await;

    plnk_with_server(&server.uri())
        .args([
            "task",
            "create",
            "--card",
            "card-1",
            "--title",
            "X",
            "--task-list",
            "Qa",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("QA (tl-a)"))
        .stderr(predicate::str::contains("qa (tl-b)"));
}

#[tokio::test]
async fn unknown_task_list_name_exits_4() {
    let server = MockServer::start().await;
    mount_card(&server, vec![task_list_json("tl-a", "Design", 65536.0)]).await;

    plnk_with_server(&server.uri())
        .args([
            "task",
            "create",
            "--card",
            "card-1",
            "--title",
            "X",
            "--task-list",
            "Release",
        ])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("task-list list --card card-1"));
}

#[tokio::test]
async fn list_with_task_list_filter_uses_task_list_endpoint() {
    let server = MockServer::start().await;
    mount_card(
        &server,
        vec![
            task_list_json("tl-a", "Design", 65536.0),
            task_list_json("tl-b", "QA", 131_072.0),
        ],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/api/task-lists/tl-b"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": task_list_json("tl-b", "QA", 131_072.0),
            "included": {"tasks": [task_item("task-7", "tl-b")["item"]]}
        })))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "task",
            "list",
            "--card",
            "card-1",
            "--task-list",
            "QA",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": \"task-7\""));
}

#[tokio::test]
async fn task_list_table_shows_task_list_column() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": card_snapshot(&[])["item"],
            "included": {
                "taskLists": [task_list_json("tl-a", "Design", 65536.0)],
                "tasks": [task_item("task-7", "tl-a")["item"]],
                "cardLabels": [], "cardMemberships": [], "attachments": []
            }
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["task", "list", "--card", "card-1", "--no-color"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Task List"))
        .stdout(predicate::str::contains("tl-a"));
}
