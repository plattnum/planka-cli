//! E2E tests: error handling, exit codes, structured error envelopes.

use assert_cmd::Command;
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

// ─── Exit code 3: auth failure ──────────────────────────────────────

#[test]
fn missing_auth_exits_3() {
    plnk()
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["project", "list"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn missing_auth_json_structured_error() {
    let output = plnk()
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["project", "list", "--output", "json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["type"], "AuthenticationFailed");
    assert!(json["error"]["message"].is_string());
}

#[tokio::test]
async fn invalid_token_exits_3() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["project", "list"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn invalid_token_json_envelope() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["project", "list", "--output", "json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["type"], "AuthenticationFailed");
}

// ─── Exit code 4: not found ────────────────────────────────────────

#[tokio::test]
async fn not_found_exits_4() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/nonexistent"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["project", "get", "nonexistent"])
        .assert()
        .failure()
        .code(4);
}

#[tokio::test]
async fn not_found_json_envelope() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/nonexistent"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["project", "get", "nonexistent", "--output", "json"])
        .assert()
        .failure()
        .code(4)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["type"], "ResourceNotFound");
}

// ─── Exit code 5: server error ─────────────────────────────────────

#[tokio::test]
async fn server_error_exits_5() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"message": "Internal Server Error"})),
        )
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["project", "list"])
        .assert()
        .failure()
        .code(5);
}

#[tokio::test]
async fn server_error_json_envelope() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"message": "Internal Server Error"})),
        )
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["project", "list", "--output", "json"])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["type"], "ApiError");
}

// ─── Exit code 2: invalid args ─────────────────────────────────────

#[test]
fn board_list_missing_scope_exits_2() {
    // board list requires --project
    plnk_with_server("http://unused:9999")
        .args(["board", "list"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn card_list_missing_scope_exits_2() {
    // card list requires --list
    plnk_with_server("http://unused:9999")
        .args(["card", "list"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn task_list_missing_scope_exits_2() {
    // task list requires --card
    plnk_with_server("http://unused:9999")
        .args(["task", "list"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn unknown_subcommand_exits_2() {
    plnk_with_server("http://unused:9999")
        .args(["project", "bogus"])
        .assert()
        .failure()
        .code(2);
}
