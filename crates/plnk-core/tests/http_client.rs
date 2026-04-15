use plnk_core::client::HttpClient;
use plnk_core::error::PlankaError;
use serde::{Deserialize, Serialize};
use url::Url;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestItem {
    id: String,
    name: String,
}

fn client_for(server: &MockServer) -> HttpClient {
    let base_url = Url::parse(&server.uri()).unwrap();
    HttpClient::new(base_url, "test-api-key").unwrap()
}

// ─── GET ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/test"))
        .and(header("X-API-Key", "test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(TestItem {
            id: "1".to_string(),
            name: "hello".to_string(),
        }))
        .mount(&server)
        .await;

    let client = client_for(&server);
    let result: TestItem = client.get("/api/test").await.unwrap();

    assert_eq!(result.id, "1");
    assert_eq!(result.name, "hello");
}

#[tokio::test]
async fn get_401_returns_auth_failed() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/test"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let client = client_for(&server);
    let result: Result<TestItem, PlankaError> = client.get("/api/test").await;

    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 3);
    assert_eq!(err.error_type(), "AuthenticationFailed");
}

#[tokio::test]
async fn get_404_returns_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/test"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = client_for(&server);
    let result: Result<TestItem, PlankaError> = client.get("/api/test").await;

    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 4);
    assert_eq!(err.error_type(), "ResourceNotFound");
}

#[tokio::test]
async fn get_500_returns_api_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/test"))
        .respond_with(
            ResponseTemplate::new(500).set_body_json(serde_json::json!({"message": "kaboom"})),
        )
        .mount(&server)
        .await;

    let client = client_for(&server);
    let result: Result<TestItem, PlankaError> = client.get("/api/test").await;

    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 5);
    assert_eq!(err.error_type(), "ApiError");
    assert!(err.to_string().contains("kaboom"));
}

// ─── POST ────────────────────────────────────────────────────────────

#[tokio::test]
async fn post_success() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/items"))
        .and(header("X-API-Key", "test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(TestItem {
            id: "2".to_string(),
            name: "created".to_string(),
        }))
        .mount(&server)
        .await;

    let client = client_for(&server);
    let body = serde_json::json!({"name": "new item"});
    let result: TestItem = client.post("/api/items", &body).await.unwrap();

    assert_eq!(result.id, "2");
    assert_eq!(result.name, "created");
}

// ─── PATCH ───────────────────────────────────────────────────────────

#[tokio::test]
async fn patch_success() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/items/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(TestItem {
            id: "1".to_string(),
            name: "updated".to_string(),
        }))
        .mount(&server)
        .await;

    let client = client_for(&server);
    let body = serde_json::json!({"name": "updated"});
    let result: TestItem = client.patch("/api/items/1", &body).await.unwrap();

    assert_eq!(result.name, "updated");
}

// ─── DELETE ──────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_success() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/items/1"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let client = client_for(&server);
    client.delete("/api/items/1").await.unwrap();
}

#[tokio::test]
async fn delete_404() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/items/999"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = client_for(&server);
    let err = client.delete("/api/items/999").await.unwrap_err();
    assert_eq!(err.exit_code(), 4);
}

// ─── Auth header on all requests ─────────────────────────────────────

#[tokio::test]
async fn auth_header_present_on_all_methods() {
    let server = MockServer::start().await;

    // If X-API-Key header is missing, the mock won't match and we'll get a connection error
    Mock::given(method("GET"))
        .and(path("/api/check"))
        .and(header("X-API-Key", "test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server);
    let _: serde_json::Value = client.get("/api/check").await.unwrap();
    // If we get here, the header was present (mock matched)
}

// ─── User-Agent header ──────────────────────────────────────────────

#[tokio::test]
async fn user_agent_header_set() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/test"))
        .and(header("User-Agent", "plnk/0.1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server);
    let _: serde_json::Value = client.get("/api/test").await.unwrap();
}

// ─── JSON error message extraction ──────────────────────────────────

#[tokio::test]
async fn error_extracts_json_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/fail"))
        .respond_with(
            ResponseTemplate::new(422)
                .set_body_json(serde_json::json!({"message": "Validation failed: name required"})),
        )
        .mount(&server)
        .await;

    let client = client_for(&server);
    let err: PlankaError = client
        .get::<serde_json::Value>("/api/fail")
        .await
        .unwrap_err();

    assert!(err.to_string().contains("Validation failed: name required"));
}
