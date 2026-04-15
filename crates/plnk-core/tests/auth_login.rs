use plnk_core::auth::{login, validate_token};
use url::Url;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn login_success() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/access-tokens"))
        .and(body_json(serde_json::json!({
            "email": "test@example.com",
            "password": "secret123"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": "returned-token-abc"
        })))
        .mount(&server)
        .await;

    let server_url = Url::parse(&server.uri()).unwrap();
    let token = login(&server_url, "test@example.com", "secret123")
        .await
        .unwrap();

    assert_eq!(token, "returned-token-abc");
}

#[tokio::test]
async fn login_invalid_credentials() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/access-tokens"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let server_url = Url::parse(&server.uri()).unwrap();
    let err = login(&server_url, "bad@example.com", "wrong")
        .await
        .unwrap_err();

    assert_eq!(err.exit_code(), 3);
    assert_eq!(err.error_type(), "AuthenticationFailed");
}

#[tokio::test]
async fn login_server_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/access-tokens"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let server_url = Url::parse(&server.uri()).unwrap();
    let err = login(&server_url, "test@example.com", "secret")
        .await
        .unwrap_err();

    assert_eq!(err.exit_code(), 5);
}

#[tokio::test]
async fn validate_token_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "123",
                "name": "Test User",
                "username": "testuser",
                "email": "test@example.com",
                "role": "editor",
                "isDeactivated": false,
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": null
            },
            "included": {}
        })))
        .mount(&server)
        .await;

    let server_url = Url::parse(&server.uri()).unwrap();
    let user = validate_token(&server_url, "valid-token").await.unwrap();

    assert_eq!(user.id, "123");
    assert_eq!(user.name, "Test User");
    assert_eq!(user.email, Some("test@example.com".to_string()));
}

#[tokio::test]
async fn validate_token_invalid() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let server_url = Url::parse(&server.uri()).unwrap();
    let err = validate_token(&server_url, "bad-token").await.unwrap_err();

    assert_eq!(err.exit_code(), 3);
    assert_eq!(err.error_type(), "AuthenticationFailed");
}
