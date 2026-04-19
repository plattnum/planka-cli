use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn plnk() -> Command {
    Command::cargo_bin("plnk").unwrap()
}

#[derive(Clone)]
struct FlakyResponder {
    attempts: Arc<AtomicUsize>,
    first: ResponseTemplate,
    rest: ResponseTemplate,
}

impl Respond for FlakyResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            self.first.clone()
        } else {
            self.rest.clone()
        }
    }
}

#[test]
fn env_transport_override_beats_config_and_allows_retry() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let server = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));

        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .and(header("X-API-Key", "test-token"))
            .respond_with(FlakyResponder {
                attempts: Arc::clone(&attempts),
                first: ResponseTemplate::new(503),
                rest: ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "item": {
                        "id": "123",
                        "name": "Retry User",
                        "username": "retry-user",
                        "email": "retry@example.com",
                        "role": "editor",
                        "isDeactivated": false,
                        "createdAt": "2026-04-14T12:00:00Z",
                        "updatedAt": null
                    },
                    "included": {}
                })),
            })
            .expect(2)
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        std::fs::write(
            &config_path,
            format!(
                "server = \"{}\"\ntoken = \"test-token\"\n\n[http]\nretry_attempts = 0\nretry_base_delay_ms = 1\nretry_max_delay_ms = 1\n",
                server.uri()
            ),
        )
        .unwrap();

        let output = plnk()
            .env("PLANKA_CONFIG", config_path.to_str().unwrap())
            .env_remove("PLANKA_SERVER")
            .env_remove("PLANKA_TOKEN")
            .env("PLNK_RETRY_ATTEMPTS", "1")
            .env("PLNK_RETRY_BASE_DELAY_MS", "1")
            .env("PLNK_RETRY_MAX_DELAY_MS", "1")
            .args(["auth", "whoami", "--output", "json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(json["data"]["name"], "Retry User");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn cli_no_retry_overrides_env_and_prevents_retry() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let server = MockServer::start().await;
        let attempts = Arc::new(AtomicUsize::new(0));

        Mock::given(method("GET"))
            .and(path("/api/users/me"))
            .and(header("X-API-Key", "test-token"))
            .respond_with(FlakyResponder {
                attempts: Arc::clone(&attempts),
                first: ResponseTemplate::new(503),
                rest: ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "item": {
                        "id": "123",
                        "name": "Should Not Retry",
                        "username": "no-retry",
                        "email": "noretry@example.com",
                        "role": "editor",
                        "isDeactivated": false,
                        "createdAt": "2026-04-14T12:00:00Z",
                        "updatedAt": null
                    },
                    "included": {}
                })),
            })
            .expect(1)
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        std::fs::write(
            &config_path,
            format!(
                "server = \"{}\"\ntoken = \"test-token\"\n\n[http]\nretry_attempts = 2\nretry_base_delay_ms = 1\nretry_max_delay_ms = 1\n",
                server.uri()
            ),
        )
        .unwrap();

        plnk()
            .env("PLANKA_CONFIG", config_path.to_str().unwrap())
            .env_remove("PLANKA_SERVER")
            .env_remove("PLANKA_TOKEN")
            .env("PLNK_RETRY_ATTEMPTS", "2")
            .args(["auth", "whoami", "--no-retry", "--output", "json"])
            .assert()
            .failure()
            .code(5)
            .stdout(predicate::str::contains("ApiError"));

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn invalid_transport_env_exits_2_before_command_runs() {
    plnk()
        .env("PLNK_HTTP_RATE_LIMIT", "0")
        .args(["auth", "logout", "--output", "json"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::contains("InvalidOptionValue"));
}

#[test]
fn token_set_preserves_http_block_in_config() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    std::fs::write(
        &config_path,
        "server = \"http://example.com\"\ntoken = \"old-token\"\n\n[http]\nmax_in_flight = 9\nrate_limit = 12\nretry_attempts = 3\n",
    )
    .unwrap();

    plnk()
        .env("PLANKA_CONFIG", config_path.to_str().unwrap())
        .env_remove("PLANKA_SERVER")
        .env_remove("PLANKA_TOKEN")
        .args(["auth", "token", "set", "new-token"])
        .assert()
        .success();

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("new-token"));
    assert!(content.contains("[http]"));
    assert!(content.contains("max_in_flight = 9"));
    assert!(content.contains("rate_limit = 12"));
    assert!(content.contains("retry_attempts = 3"));
}
