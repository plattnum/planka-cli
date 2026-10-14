//! E2E tests for the `--names` mode plumbing.
//!
//! Resource-specific resolution and decoration live in the commits that add
//! each resource. This file covers the mode flags themselves.

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn plnk() -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env_remove("PLNK_PROJECT");
    cmd.env_remove("PLNK_BOARD");
    cmd.env_remove("PLNK_LIST");
    cmd
}

fn plnk_with_server(server_uri: &str) -> Command {
    let mut cmd = plnk();
    cmd.env("PLANKA_SERVER", server_uri);
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd
}

#[test]
fn names_and_no_names_conflict() {
    plnk()
        .args(["--names", "--no-names", "project", "list"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
}

#[tokio::test]
async fn names_flag_does_not_change_a_command_without_mappings() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [{"id":"1","name":"P","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}]
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["--names", "project", "list", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"][0]["name"], "P");
}

// ─── Project: input resolution, JSON decoration, text substitution ────────

fn project_list_body() -> serde_json::Value {
    serde_json::json!({
        "items": [
            {"id":"1","name":"Platform","createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
        ]
    })
}

fn project_snapshot_body() -> serde_json::Value {
    serde_json::json!({
        "item": {"id":"1","name":"Platform","createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
        "included": {
            "boards": [
                {"id":"10","projectId":"1","name":"Sprint","position":65536,
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]
        }
    })
}

#[tokio::test]
async fn project_name_resolves_and_decorates_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_list_body()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_snapshot_body()))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names",
            "board",
            "list",
            "--project",
            "Platform",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"][0]["projectId"], "1");
    assert_eq!(json["data"][0]["projectName"], "Platform");
}

#[tokio::test]
async fn project_name_is_substituted_in_table() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_list_body()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_snapshot_body()))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["--names", "board", "list", "--project", "Platform"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Platform"))
        .stdout(predicate::str::contains("| 1 |").not());
}

#[tokio::test]
async fn ambiguous_project_name_lists_candidates() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [
                {"id":"1","name":"Platform One","createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
                {"id":"2","name":"Platform Two","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["--names", "board", "list", "--project", "Platform"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Platform One (1)"))
        .stderr(predicate::str::contains("Platform Two (2)"));
}

// ─── Board: input resolution, JSON decoration, text substitution ─────────

fn project_snapshot_with_boards(boards: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "item": {"id":"1","name":"Platform","createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
        "included": { "boards": boards }
    })
}

fn board_snapshot_body() -> serde_json::Value {
    serde_json::json!({
        "item": {"id":"10","name":"Sprint","position":65536,"projectId":"1",
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
        "included": {
            "lists": [
                {"id":"100","boardId":"10","name":"Todo","position":65536,
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]
        }
    })
}

#[tokio::test]
async fn board_name_resolves_and_decorates_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_list_body()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_snapshot_with_boards(
                serde_json::json!([
                    {"id":"10","projectId":"1","name":"Sprint","position":65536,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
                ]),
            )),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/boards/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot_body()))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names", "list", "list", "--board", "Sprint", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"][0]["boardId"], "10");
    assert_eq!(json["data"][0]["boardName"], "Sprint");
    assert_eq!(json["data"][0]["projectName"], "Platform");
}

// ─── List: input resolution, JSON decoration, text substitution ──────────

fn card_list_body() -> serde_json::Value {
    serde_json::json!({
        "items": [
            {"id":"1000","listId":"100","boardId":"10","name":"Task",
             "position":65536,"isClosed":false,
             "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
        ]
    })
}

async fn mount_list_discovery(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_list_body()))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_snapshot_with_boards(
                serde_json::json!([
                    {"id":"10","projectId":"1","name":"Sprint","position":65536,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
                ]),
            )),
        )
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/boards/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot_body()))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/lists/100/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(card_list_body()))
        .mount(server)
        .await;
}

#[tokio::test]
async fn list_name_resolves_and_decorates_json() {
    let server = MockServer::start().await;
    mount_list_discovery(&server).await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names", "card", "list", "--list", "Todo", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"][0]["listId"], "100");
    assert_eq!(json["data"][0]["listName"], "Todo");
    assert_eq!(json["data"][0]["projectName"], "Platform");
}

#[tokio::test]
async fn list_name_is_substituted_in_table() {
    let server = MockServer::start().await;
    mount_list_discovery(&server).await;

    plnk_with_server(&server.uri())
        .args(["--names", "card", "list", "--list", "Todo"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Todo"));
}

// ─── User: input resolution and JSON decoration ──────────────────────────

fn user_body() -> serde_json::Value {
    serde_json::json!({
        "items": [
            {"id":"7","name":"Alice Smith","username":"alice","role":"editor",
             "isDeactivated":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
        ]
    })
}

#[tokio::test]
async fn username_resolves_for_assignee_add() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(user_body()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/cards/1000/card-memberships"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"m1","cardId":"1000","userId":"7",
                     "createdAt":"2026-01-01T00:00:00Z"}
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names", "card", "assignee", "add", "1000", "alice", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["userId"], "7");
    assert_eq!(json["data"]["userName"], "alice");
}

#[tokio::test]
async fn comment_user_is_decorated_with_username() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/cards/1000/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [
                {"id":"c1","cardId":"1000","userId":"7","text":"hi",
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/users/7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"7","name":"Alice Smith","username":"alice","role":"editor",
                     "isDeactivated":false,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names", "comment", "list", "--card", "1000", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"][0]["userName"], "alice");
}

// ─── Label: input resolution and JSON decoration ─────────────────────────

fn board_snapshot_with_labels() -> serde_json::Value {
    serde_json::json!({
        "item": {"id":"10","name":"Sprint","position":65536,"projectId":"1",
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
        "included": {
            "labels": [
                {"id":"50","boardId":"10","name":"urgent","color":"berry-red","position":65536,
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]
        }
    })
}

#[tokio::test]
async fn label_name_resolves_and_decorates_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_list_body()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_snapshot_with_boards(
                serde_json::json!([
                    {"id":"10","projectId":"1","name":"Sprint","position":65536,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
                ]),
            )),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/boards/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot_with_labels()))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/cards/1000/card-labels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"cl1","cardId":"1000","labelId":"50",
                     "createdAt":"2026-01-01T00:00:00Z"}
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names", "card", "label", "add", "1000", "urgent", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["labelId"], "50");
    assert_eq!(json["data"]["labelName"], "urgent");
}

// ─── Custom field group: input resolution and JSON decoration ────────────

fn projects_with_base_group() -> serde_json::Value {
    serde_json::json!({
        "items": [
            {"id":"1","name":"Platform","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
        ],
        "included": {
            "baseCustomFieldGroups": [
                {"id":"bg1","projectId":"1","name":"Template",
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ],
            "customFields": []
        }
    })
}

fn empty_board_snapshot() -> serde_json::Value {
    serde_json::json!({
        "item": {"id":"10","name":"Sprint","position":65536,"projectId":"1",
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
        "included": {}
    })
}

async fn mount_group_discovery(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(projects_with_base_group()))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_snapshot_with_boards(
                serde_json::json!([
                    {"id":"10","projectId":"1","name":"Sprint","position":65536,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
                ]),
            )),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn group_name_resolves_and_falls_back_to_base() {
    let server = MockServer::start().await;
    mount_group_discovery(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/boards/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_board_snapshot()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/custom-field-groups/bg1"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names",
            "field-group",
            "get",
            "Template",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["id"], "bg1");
    assert_eq!(json["data"]["name"], "Template");
    assert_eq!(json["data"]["projectName"], "Platform");
}

// ─── Custom field: JSON decoration ───────────────────────────────────────

#[tokio::test]
async fn field_reference_is_decorated() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/cards/1000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"1000","listId":"100","boardId":"10","name":"Task",
                     "position":65536,"isClosed":false,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {"customFieldValues": [
                {"id":"v1","cardId":"1000","customFieldGroupId":"g1","customFieldId":"f1",
                 "content":"high","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/custom-field-groups/g1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"g1","name":"Priority","boardId":"10","position":65536,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {"customFields": [
                {"id":"f1","name":"Priority Level","position":65536,
                 "showOnFrontOfCard":false,"customFieldGroupId":"g1",
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [
                {"id":"1","name":"Platform","createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ],
            "included": {"baseCustomFieldGroups": [], "customFields": []}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_snapshot_with_boards(
                serde_json::json!([
                    {"id":"10","projectId":"1","name":"Sprint","position":65536,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
                ]),
            )),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/boards/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"10","name":"Sprint","position":65536,"projectId":"1",
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {"customFieldGroups": [
                {"id":"g1","name":"Priority","boardId":"10","position":65536,
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]}
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names", "card", "field", "list", "1000", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"][0]["customFieldId"], "f1");
    assert_eq!(json["data"][0]["customFieldName"], "Priority Level");
}

#[tokio::test]
async fn group_references_are_decorated() {
    let server = MockServer::start().await;
    mount_group_discovery(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/boards/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"10","name":"Sprint","position":65536,"projectId":"1",
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {"customFieldGroups": [
                {"id":"g1","name":null,"boardId":"10","baseCustomFieldGroupId":"bg1",
                 "position":65536,"createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]}
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "--names",
            "field-group",
            "list",
            "--board",
            "Sprint",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"][0]["baseCustomFieldGroupId"], "bg1");
    assert_eq!(json["data"][0]["baseCustomFieldGroupName"], "Template");
}

#[tokio::test]
async fn ambiguous_list_name_lists_candidates() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_list_body()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_snapshot_with_boards(
                serde_json::json!([
                    {"id":"10","projectId":"1","name":"Sprint","position":65536,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
                ]),
            )),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/boards/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {"id":"10","name":"Sprint","position":65536,"projectId":"1",
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
            "included": {"lists": [
                {"id":"100","boardId":"10","name":"Todo One","position":65536,
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
                {"id":"101","boardId":"10","name":"Todo Two","position":131072,
                 "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
            ]}
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["--names", "card", "list", "--list", "Todo"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Todo One (100)"))
        .stderr(predicate::str::contains("Todo Two (101)"));
}

#[tokio::test]
async fn ambiguous_board_name_lists_candidates() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_list_body()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(project_snapshot_with_boards(
                serde_json::json!([
                    {"id":"10","projectId":"1","name":"Sprint One","position":65536,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null},
                    {"id":"11","projectId":"1","name":"Sprint Two","position":131072,
                     "createdAt":"2026-01-01T00:00:00Z","updatedAt":null}
                ]),
            )),
        )
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["--names", "list", "list", "--board", "Sprint"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Sprint One (10)"))
        .stderr(predicate::str::contains("Sprint Two (11)"));
}

#[tokio::test]
async fn unknown_project_name_exits_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_list_body()))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["--names", "board", "list", "--project", "Nope"])
        .assert()
        .failure()
        .code(4);
}
