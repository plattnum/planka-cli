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
    // Bare dates resolve to local midnight; pin UTC so the expected UTC body
    // does not depend on the machine's timezone.
    cmd.env("TZ", "UTC");
    cmd
}

fn card_body(due: serde_json::Value) -> serde_json::Value {
    json!({
        "item": {
            "id": "card-1",
            "listId": "list-1",
            "boardId": "board-1",
            "name": "Fix auth",
            "description": null,
            "position": 65536.0,
            "dueDate": due,
            "isClosed": false,
            "isSubscribed": false,
            "createdAt": "2026-01-01T12:00:00Z",
            "updatedAt": null
        }
    })
}

#[tokio::test]
async fn card_update_sets_due_date_as_utc() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/cards/card-1"))
        .and(body_json(json!({"dueDate": "2026-04-20T00:00:00Z"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(card_body(json!("2026-04-20T00:00:00.000Z"))),
        )
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card",
            "update",
            "card-1",
            "--due-date",
            "2026-04-20",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"]["dueDate"], "2026-04-20T00:00:00.000Z");
}

/// A bare date is midnight in the user's timezone before normalisation. Pin a
/// west-of-UTC zone in DST so the UTC instant is the day before: midnight EDT
/// (UTC-4) on 20 April is 04:00Z.
#[tokio::test]
async fn card_update_bare_due_date_is_local_midnight() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/cards/card-1"))
        .and(body_json(json!({"dueDate": "2026-04-20T04:00:00Z"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(card_body(json!("2026-04-20T04:00:00.000Z"))),
        )
        .mount(&server)
        .await;

    let mut cmd = plnk_with_server(&server.uri());
    cmd.env("TZ", "EST5EDT");
    cmd.args([
        "card",
        "update",
        "card-1",
        "--due-date",
        "2026-04-20",
        "--output",
        "json",
    ])
    .assert()
    .success();
}

#[tokio::test]
async fn card_update_normalizes_offset_to_utc() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/cards/card-1"))
        .and(body_json(json!({"dueDate": "2026-04-20T00:30:00Z"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(card_body(json!("2026-04-20T00:30:00.000Z"))),
        )
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "update",
            "card-1",
            "--due-date",
            "2026-04-20T02:30:00+02:00",
            "--output",
            "json",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn card_update_sends_title_and_due_date_together() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/cards/card-1"))
        .and(body_json(
            json!({"name": "Fix auth race", "dueDate": "2026-04-20T00:00:00Z"}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(card_body(json!("2026-04-20T00:00:00.000Z"))),
        )
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "update",
            "card-1",
            "--title",
            "Fix auth race",
            "--due-date",
            "2026-04-20",
            "--output",
            "json",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn card_update_clear_due_date_sends_null() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/cards/card-1"))
        .and(body_json(json!({"dueDate": null})))
        .respond_with(ResponseTemplate::new(200).set_body_json(card_body(serde_json::Value::Null)))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "update",
            "card-1",
            "--clear-due-date",
            "--output",
            "json",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn card_update_rejects_due_date_and_clear_together() {
    let server = MockServer::start().await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "update",
            "card-1",
            "--due-date",
            "2026-04-20",
            "--clear-due-date",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--due-date"));
}

#[tokio::test]
async fn card_update_invalid_due_date_exits_2() {
    let server = MockServer::start().await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card",
            "update",
            "card-1",
            "--due-date",
            "not-a-date",
            "--output",
            "json",
        ])
        .assert()
        .failure()
        .code(2)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["error"]["type"], "InvalidOptionValue");
    assert_eq!(json["error"]["field"], "--due-date");
}

#[tokio::test]
async fn card_update_requires_a_field_to_change() {
    let server = MockServer::start().await;

    plnk_with_server(&server.uri())
        .args(["card", "update", "card-1", "--output", "json"])
        .assert()
        .failure()
        .code(2);
}
