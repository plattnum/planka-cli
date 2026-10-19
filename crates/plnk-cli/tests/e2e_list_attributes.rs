//! End-to-end tests for list type and color on `plnk list create` / `update`.
//!
//! Two server behaviours shape these tests: `POST /api/boards/{id}/lists`
//! silently drops `color`, so a colored create is two requests, and a `closed`
//! list is what marks cards done, so `list list` must make it identifiable.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn plnk_with_server(server_uri: &str) -> Command {
    let mut cmd = Command::cargo_bin("plnk").unwrap();
    cmd.env("PLANKA_CONFIG", "/tmp/plnk-test-nonexistent/config.toml");
    cmd.env("PLANKA_SERVER", server_uri);
    cmd.env("PLANKA_TOKEN", "test-api-key");
    cmd
}

fn list_json(id: &str, name: &str, list_type: &str, color: Option<&str>) -> serde_json::Value {
    json!({
        "id": id,
        "boardId": "board-1",
        "name": name,
        "position": 65536.0,
        "type": list_type,
        "color": color,
        "createdAt": "2026-10-19T00:00:00Z",
        "updatedAt": null
    })
}

#[tokio::test]
async fn create_closed_list_sends_type_closed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/boards/board-1/lists"))
        .and(body_json(json!({
            "boardId": "board-1",
            "name": "Done",
            "type": "closed",
            "position": 65536.0
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"item": list_json("list-1", "Done", "closed", None)})),
        )
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "list", "create", "--board", "board-1", "--name", "Done", "--type", "closed",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("closed"));
}

#[tokio::test]
async fn create_defaults_to_active_and_honours_position() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/boards/board-1/lists"))
        .and(body_json(json!({
            "boardId": "board-1",
            "name": "Doing",
            "type": "active",
            "position": 0.0
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"item": list_json("list-1", "Doing", "active", None)})),
        )
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "list",
            "create",
            "--board",
            "board-1",
            "--name",
            "Doing",
            "--position",
            "top",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn create_with_color_posts_then_patches() {
    let server = MockServer::start().await;
    // The POST body carries no color: the server would drop it anyway.
    Mock::given(method("POST"))
        .and(path("/api/boards/board-1/lists"))
        .and(body_json(json!({
            "boardId": "board-1",
            "name": "Blocked",
            "type": "active",
            "position": 65536.0
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"item": list_json("list-9", "Blocked", "active", None)})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/lists/list-9"))
        .and(body_json(json!({"color": "berry-red"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"item": list_json("list-9", "Blocked", "active", Some("berry-red"))}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "list",
            "create",
            "--board",
            "board-1",
            "--name",
            "Blocked",
            "--color",
            "berry-red",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // The rendered list is the PATCH result, so the color is present.
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["color"], "berry-red");
}

#[tokio::test]
async fn create_with_color_patch_failure_reports_created_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/boards/board-1/lists"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"item": list_json("list-9", "Blocked", "active", None)})),
        )
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/lists/list-9"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "code": "E_INTERNAL",
            "message": "boom"
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "list",
            "create",
            "--board",
            "board-1",
            "--name",
            "Blocked",
            "--color",
            "berry-red",
            "--no-retry",
        ])
        .assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains(
            "list list-9 was created but setting its color failed",
        ));
}

#[tokio::test]
async fn update_sets_type_and_color() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/lists/list-1"))
        .and(body_json(json!({"type": "closed", "color": "lagoon-blue"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"item": list_json("list-1", "Done", "closed", Some("lagoon-blue"))}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "list",
            "update",
            "list-1",
            "--type",
            "closed",
            "--color",
            "lagoon-blue",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn update_clear_color_sends_null() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/lists/list-1"))
        .and(body_json(json!({"color": null})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"item": list_json("list-1", "Done", "closed", None)})),
        )
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["list", "update", "list-1", "--clear-color"])
        .assert()
        .success();
}

#[tokio::test]
async fn invalid_color_or_type_exits_2_without_request() {
    let server = MockServer::start().await;
    // No mocks: any request would 404 and fail with a different exit code.
    for args in [
        vec!["list", "update", "list-1", "--color", "neon-green"],
        vec!["list", "update", "list-1", "--type", "archive"],
        vec![
            "list",
            "update",
            "list-1",
            "--color",
            "berry-red",
            "--clear-color",
        ],
        vec![
            "list", "create", "--board", "board-1", "--name", "X", "--color", "red",
        ],
    ] {
        plnk_with_server(&server.uri())
            .args(&args)
            .assert()
            .failure()
            .code(2);
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn list_list_table_shows_type_column() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": "board-1",
                "name": "Work",
                "position": 65536.0,
                "projectId": "project-1",
                "createdAt": "2026-10-19T00:00:00Z",
                "updatedAt": null
            },
            "included": {
                "lists": [
                    list_json("list-1", "Doing", "active", None),
                    list_json("list-2", "Done", "closed", Some("bright-moss")),
                ]
            }
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["list", "list", "--board", "board-1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Type"))
        .stdout(predicate::str::contains("closed"))
        .stdout(predicate::str::contains("bright-moss"));
}
