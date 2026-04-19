use std::time::Duration;

use plnk_core::api::{CardApi, PlankaClientV1};
use plnk_core::client::HttpClient;
use plnk_core::transport::TransportPolicy;
use serde_json::json;
use url::Url;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn client_for(server: &MockServer, policy: TransportPolicy) -> PlankaClientV1 {
    let base_url = Url::parse(&server.uri()).unwrap();
    let http = HttpClient::with_policy(base_url, "test-api-key", policy).unwrap();
    PlankaClientV1::new(http)
}

fn card_response(id: &str) -> serde_json::Value {
    json!({
        "item": {
            "id": id,
            "listId": "list-1",
            "boardId": "board-1",
            "name": format!("Card {id}"),
            "description": null,
            "position": 65536.0,
            "isClosed": false,
            "isSubscribed": false,
            "createdAt": "2026-04-19T00:00:00Z",
            "updatedAt": null
        }
    })
}

#[derive(Clone)]
struct DelayedCardResponder {
    id: &'static str,
    delay_ms: u64,
}

impl Respond for DelayedCardResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        std::thread::sleep(Duration::from_millis(self.delay_ms));
        ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": self.id,
                "listId": "list-1",
                "boardId": "board-1",
                "name": format!("Card {}", self.id),
                "description": null,
                "position": 65536.0,
                "isClosed": false,
                "isSubscribed": false,
                "createdAt": "2026-04-19T00:00:00Z",
                "updatedAt": null
            }
        }))
    }
}

#[tokio::test]
async fn get_many_preserves_input_order_under_parallelism() {
    let server = MockServer::start().await;

    for (id, delay_ms) in [("card-1", 180), ("card-2", 20), ("card-3", 90)] {
        Mock::given(method("GET"))
            .and(path(format!("/api/cards/{id}")))
            .and(header("X-API-Key", "test-api-key"))
            .respond_with(DelayedCardResponder { id, delay_ms })
            .expect(1)
            .mount(&server)
            .await;
    }

    let client = client_for(&server, TransportPolicy::default());
    let result = client
        .get_many_cards(
            vec![
                "card-1".to_string(),
                "card-2".to_string(),
                "card-3".to_string(),
            ],
            3,
        )
        .await
        .unwrap();

    let ids = result
        .cards
        .iter()
        .map(|card| card.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["card-1", "card-2", "card-3"]);
    assert_eq!(result.concurrency, 3);
    assert!(result.missing_ids.is_empty());
    assert!(result.failures.is_empty());
}

#[tokio::test]
async fn get_many_caps_local_parallelism_to_transport_max_in_flight() {
    let server = MockServer::start().await;

    for id in ["card-1", "card-2", "card-3", "card-4"] {
        Mock::given(method("GET"))
            .and(path(format!("/api/cards/{id}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(75))
                    .set_body_json(card_response(id)),
            )
            .expect(1)
            .mount(&server)
            .await;
    }

    let client = client_for(
        &server,
        TransportPolicy {
            max_in_flight: 2,
            rate_limit_per_second: None,
            burst_size: None,
            retry_attempts: 0,
            ..TransportPolicy::default()
        },
    );

    let start = std::time::Instant::now();
    let result = client
        .get_many_cards(
            vec![
                "card-1".to_string(),
                "card-2".to_string(),
                "card-3".to_string(),
                "card-4".to_string(),
            ],
            4,
        )
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert_eq!(result.concurrency, 2);
    assert!(
        elapsed >= Duration::from_millis(140) && elapsed < Duration::from_millis(260),
        "unexpected elapsed time for two-at-a-time execution: {elapsed:?}"
    );
}
