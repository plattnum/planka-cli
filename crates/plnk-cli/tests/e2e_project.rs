//! E2E tests: project CRUD lifecycle via wiremock mock server.

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

// ─── Project list ───────────────────────────────────────────────────

#[tokio::test]
async fn project_list_json() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [
                {
                    "id": "1",
                    "name": "Alpha",
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                },
                {
                    "id": "2",
                    "name": "Beta",
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]
        })))
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
    assert_eq!(json["meta"]["count"], 2);
    assert_eq!(json["data"][0]["name"], "Alpha");
    assert_eq!(json["data"][1]["name"], "Beta");
}

#[tokio::test]
async fn project_list_table() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": [
                {
                    "id": "1",
                    "name": "Alpha",
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["project", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Alpha"))
        .stdout(predicate::str::contains("1"));
}

// ─── Project get ────────────────────────────────────────────────────

#[tokio::test]
async fn project_get_json() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "42",
                "name": "Platform",
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": null
            },
            "included": {
                "boards": [],
                "projectManagers": []
            }
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["project", "get", "42", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["name"], "Platform");
}

// ─── Project create ─────────────────────────────────────────────────

#[tokio::test]
async fn project_create_json() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "99",
                "name": "New Project",
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": null
            }
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "project",
            "create",
            "--name",
            "New Project",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["name"], "New Project");
    assert_eq!(json["data"]["id"], "99");
}

// ─── Project update ─────────────────────────────────────────────────

#[tokio::test]
async fn project_update_json() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "42",
                "name": "Renamed",
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": "2026-04-15T12:00:00Z"
            }
        })))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "project", "update", "42", "--name", "Renamed", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["name"], "Renamed");
}

// ─── Project delete ─────────────────────────────────────────────────

#[tokio::test]
async fn project_delete_json() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/projects/42"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["project", "delete", "42", "--yes", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\":true"));
}

// ─── Full lifecycle: list → create → get → update → delete ─────────

#[tokio::test]
async fn project_crud_lifecycle() {
    let server = MockServer::start().await;

    // Step 1: List (empty)
    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": []
        })))
        .expect(1)
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
    assert_eq!(json["meta"]["count"], 0);

    // Step 2: Create
    Mock::given(method("POST"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "new-1",
                "name": "Test Project",
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "project",
            "create",
            "--name",
            "Test Project",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["id"], "new-1");

    // Step 3: Get
    Mock::given(method("GET"))
        .and(path("/api/projects/new-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "new-1",
                "name": "Test Project",
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": null
            },
            "included": { "boards": [], "projectManagers": [] }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["project", "get", "new-1", "--output", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["name"], "Test Project");

    // Step 4: Update
    Mock::given(method("PATCH"))
        .and(path("/api/projects/new-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "new-1",
                "name": "Updated Project",
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": "2026-04-15T12:00:00Z"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "project",
            "update",
            "new-1",
            "--name",
            "Updated Project",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["name"], "Updated Project");

    // Step 5: Delete
    Mock::given(method("DELETE"))
        .and(path("/api/projects/new-1"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["project", "delete", "new-1", "--yes"])
        .assert()
        .success();
}
