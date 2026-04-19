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

fn card_response(id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "item": {
            "id": id,
            "listId": "list-1",
            "boardId": "board-1",
            "name": name,
            "description": null,
            "position": 65536.0,
            "isClosed": false,
            "isSubscribed": false,
            "createdAt": "2026-04-19T00:00:00Z",
            "updatedAt": null
        }
    })
}

#[tokio::test]
async fn get_many_json_preserves_input_order_and_reports_meta() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(150))
                .set_body_json(card_response("card-1", "Alpha")),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/cards/card-2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(10))
                .set_body_json(card_response("card-2", "Beta")),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/cards/card-3"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_millis(75))
                .set_body_json(card_response("card-3", "Gamma")),
        )
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card", "get-many", "--id", "card-1", "--id", "card-2", "--id", "card-3", "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let ids = json["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec!["card-1", "card-2", "card-3"]);
    assert_eq!(json["meta"]["count"], 3);
    assert_eq!(json["meta"]["requestedCount"], 3);
    assert_eq!(json["meta"]["foundCount"], 3);
    assert_eq!(json["meta"]["missingCount"], 0);
    assert_eq!(json["meta"]["missingIds"], serde_json::json!([]));
    assert_eq!(json["meta"]["concurrency"], 4);
    assert_eq!(json["meta"]["allowMissing"], false);
}

#[tokio::test]
async fn get_many_strict_missing_exits_4_with_aggregated_ids() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(card_response("card-1", "Alpha")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/cards/missing-1"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/cards/missing-2"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card",
            "get-many",
            "--id",
            "card-1",
            "--id",
            "missing-1",
            "--id",
            "missing-2",
            "--output",
            "json",
        ])
        .assert()
        .failure()
        .code(4)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["error"]["type"], "ResourceNotFound");
    assert_eq!(json["error"]["resource"], "card");
    assert_eq!(
        json["error"]["missingIds"],
        serde_json::json!(["missing-1", "missing-2"])
    );
    assert_eq!(json["error"]["requestedCount"], 3);
    assert_eq!(json["error"]["foundCount"], 1);
}

#[tokio::test]
async fn get_many_allow_missing_returns_found_cards_and_meta() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(card_response("card-1", "Alpha")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/cards/missing-1"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card",
            "get-many",
            "--id",
            "card-1",
            "--id",
            "missing-1",
            "--allow-missing",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
    assert_eq!(json["data"][0]["id"], "card-1");
    assert_eq!(json["meta"]["requestedCount"], 2);
    assert_eq!(json["meta"]["foundCount"], 1);
    assert_eq!(json["meta"]["missingCount"], 1);
    assert_eq!(json["meta"]["missingIds"], serde_json::json!(["missing-1"]));
    assert_eq!(json["meta"]["allowMissing"], true);
}

#[tokio::test]
async fn get_many_allow_missing_does_not_suppress_server_failures() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(card_response("card-1", "Alpha")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/cards/card-2"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"message": "Internal Server Error"})),
        )
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card",
            "get-many",
            "--id",
            "card-1",
            "--id",
            "card-2",
            "--allow-missing",
            "--output",
            "json",
        ])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["error"]["type"], "ApiError");
    assert_eq!(json["error"]["requestedCount"], 2);
    assert_eq!(json["error"]["failures"][0]["id"], "card-2");
    assert_eq!(json["error"]["failures"][0]["type"], "ApiError");
}

#[tokio::test]
async fn get_many_auth_failure_exits_3() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args(["card", "get-many", "--id", "card-1", "--output", "json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["error"]["type"], "AuthenticationFailed");
    assert_eq!(json["error"]["requestedCount"], 1);
    assert_eq!(json["error"]["failures"][0]["id"], "card-1");
}

#[test]
fn get_many_requires_at_least_one_id() {
    plnk_with_server("http://unused:9999")
        .args(["card", "get-many"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn get_many_rejects_out_of_range_concurrency() {
    plnk_with_server("http://unused:9999")
        .args(["card", "get-many", "--id", "card-1", "--concurrency", "0"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("--concurrency"));
}
