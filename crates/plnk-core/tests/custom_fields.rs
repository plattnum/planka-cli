//! Wire-level tests for the custom field API traits.
//!
//! These pin the routes and payloads that Planka's published `OpenAPI` spec gets
//! wrong, and the idempotent-clear behaviour that deliberately departs from the
//! crate's usual error handling.

use plnk_core::api::{CardCustomFieldApi, CustomFieldApi, CustomFieldGroupApi, PlankaClientV1};
use plnk_core::client::HttpClient;
use plnk_core::error::PlankaError;
use plnk_core::models::{UpdateCustomField, UpdateCustomFieldGroup};
use serde_json::json;
use url::Url;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer) -> PlankaClientV1 {
    let base_url = Url::parse(&server.uri()).unwrap();
    let http = HttpClient::new(base_url, "test-api-key").unwrap();
    PlankaClientV1::new(http)
}

/// The composite value segment carries no `$`. Planka's published spec writes
/// `customFieldId:${customFieldId}`; sending the literal `$` returns a 400.
const VALUE_PATH: &str =
    "/api/cards/card-1/custom-field-values/customFieldGroupId:group-1:customFieldId:field-1";

fn value_body() -> serde_json::Value {
    json!({
        "item": {
            "id": "value-1",
            "cardId": "card-1",
            "customFieldGroupId": "group-1",
            "customFieldId": "field-1",
            "content": "specs/x.html",
            "createdAt": "2026-08-10T00:00:00Z",
            "updatedAt": null
        }
    })
}

#[tokio::test]
async fn set_field_value_uses_composite_path_without_dollar_sign() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path(VALUE_PATH))
        .and(body_json(json!({"content": "specs/x.html"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(value_body()))
        .expect(1)
        .mount(&server)
        .await;

    let value = client_for(&server)
        .set_field_value("card-1", "group-1", "field-1", "specs/x.html")
        .await
        .unwrap();

    assert_eq!(value.id, "value-1");
    assert_eq!(value.content, "specs/x.html");
}

/// The live DELETE route is plural. The spec's singular `custom-field-value`
/// does not exist — mounting only the plural route proves we target it.
#[tokio::test]
async fn clear_field_value_targets_plural_route() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path(VALUE_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(value_body()))
        .expect(1)
        .mount(&server)
        .await;

    client_for(&server)
        .clear_field_value("card-1", "group-1", "field-1")
        .await
        .unwrap();
}

/// Clearing is "ensure unset", so an already-unset value is a success.
#[tokio::test]
async fn clear_field_value_swallows_404() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path(VALUE_PATH))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "code": "E_NOT_FOUND",
            "message": "Custom field value not found"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = client_for(&server)
        .clear_field_value("card-1", "group-1", "field-1")
        .await;

    assert!(result.is_ok(), "an already-unset value must clear cleanly");
}

/// Only 404 is swallowed — every other status stays fatal.
#[tokio::test]
async fn clear_field_value_propagates_non_404_errors() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path(VALUE_PATH))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "code": "E_INTERNAL_SERVER_ERROR"
        })))
        .mount(&server)
        .await;

    let result = client_for(&server)
        .clear_field_value("card-1", "group-1", "field-1")
        .await;

    assert!(
        matches!(result, Err(PlankaError::ApiError { status: 500, .. })),
        "a 500 must not be treated as an already-unset value, got {result:?}"
    );
}

#[tokio::test]
async fn create_card_field_group_sends_base_id_and_never_a_name() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/cards/card-1/custom-field-groups"))
        .and(body_json(json!({
            "baseCustomFieldGroupId": "base-1",
            "position": 65536.0
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": "group-1",
                "name": null,
                "boardId": null,
                "cardId": "card-1",
                "baseCustomFieldGroupId": "base-1",
                // The server repositions when a sibling holds the slot; the
                // echo is deliberately different from what we sent.
                "position": 81920.0,
                "createdAt": "2026-08-10T00:00:00Z",
                "updatedAt": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let group = client_for(&server)
        .create_card_field_group("card-1", Some("base-1"), None)
        .await
        .unwrap();

    assert!(group.name.is_none());
    assert_eq!(group.base_custom_field_group_id, Some("base-1".to_string()));
    assert!(
        (group.position - 81920.0).abs() < f64::EPSILON,
        "the server's position must be taken as authoritative, not the request's"
    );
}

#[tokio::test]
async fn create_card_field_group_sends_name_and_never_a_base_id() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/cards/card-1/custom-field-groups"))
        .and(body_json(json!({"name": "Ad-hoc", "position": 65536.0})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": "group-2",
                "name": "Ad-hoc",
                "boardId": null,
                "cardId": "card-1",
                "baseCustomFieldGroupId": null,
                "position": 65536.0,
                "createdAt": "2026-08-10T00:00:00Z",
                "updatedAt": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let group = client_for(&server)
        .create_card_field_group("card-1", None, Some("Ad-hoc"))
        .await
        .unwrap();

    assert_eq!(group.name, Some("Ad-hoc".to_string()));
    assert!(group.base_custom_field_group_id.is_none());
}

#[tokio::test]
async fn create_card_field_group_without_base_or_name_is_a_validation_error() {
    let server = MockServer::start().await;

    let result = client_for(&server)
        .create_card_field_group("card-1", None, None)
        .await;

    assert!(
        matches!(result, Err(PlankaError::MissingRequiredOption { .. })),
        "expected a validation error and no request, got {result:?}"
    );
}

/// Base groups live behind their own routes: creating a field on one uses
/// `/api/base-custom-field-groups/{id}/custom-fields`.
#[tokio::test]
async fn create_field_routes_by_group_kind() {
    let server = MockServer::start().await;

    let field = |group_key: &str, base: Option<&str>| {
        json!({
            "item": {
                "id": "field-1",
                "name": "Specification",
                "position": 65536.0,
                "showOnFrontOfCard": true,
                "customFieldGroupId": if base.is_some() { serde_json::Value::Null }
                                      else { json!(group_key) },
                "baseCustomFieldGroupId": base.map_or(serde_json::Value::Null, |b| json!(b)),
                "createdAt": "2026-08-10T00:00:00Z",
                "updatedAt": null
            }
        })
    };

    Mock::given(method("POST"))
        .and(path("/api/base-custom-field-groups/base-1/custom-fields"))
        .and(body_json(json!({
            "name": "Specification",
            "position": 65536.0,
            "showOnFrontOfCard": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(field("base-1", Some("base-1"))))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/custom-field-groups/group-1/custom-fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(field("group-1", None)))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server);
    let from_base = client
        .create_field("base-1", true, "Specification", true)
        .await
        .unwrap();
    assert!(from_base.show_on_front_of_card);
    assert_eq!(
        from_base.base_custom_field_group_id,
        Some("base-1".to_string())
    );

    let from_group = client
        .create_field("group-1", false, "Specification", true)
        .await
        .unwrap();
    assert_eq!(
        from_group.custom_field_group_id,
        Some("group-1".to_string())
    );
}

/// A base group's fields are readable only from the projects list — no board
/// snapshot and no by-id base-group route exposes them.
#[tokio::test]
async fn list_fields_for_base_group_reads_the_projects_list() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [],
            "included": {
                "baseCustomFieldGroups": [
                    {
                        "id": "base-1",
                        "name": "Documentation",
                        "projectId": "project-1",
                        "createdAt": "2026-08-10T00:00:00Z",
                        "updatedAt": null
                    }
                ],
                "customFields": [
                    {
                        "id": "field-1",
                        "name": "Specification",
                        "position": 65536.0,
                        "showOnFrontOfCard": true,
                        "customFieldGroupId": null,
                        "baseCustomFieldGroupId": "base-1",
                        "createdAt": "2026-08-10T00:00:00Z",
                        "updatedAt": null
                    },
                    {
                        "id": "field-2",
                        "name": "Belongs elsewhere",
                        "position": 65536.0,
                        "showOnFrontOfCard": false,
                        "customFieldGroupId": null,
                        "baseCustomFieldGroupId": "base-other",
                        "createdAt": "2026-08-10T00:00:00Z",
                        "updatedAt": null
                    }
                ]
            }
        })))
        .mount(&server)
        .await;

    let fields = client_for(&server)
        .list_fields("base-1", true)
        .await
        .unwrap();

    assert_eq!(fields.len(), 1, "must not leak other base groups' fields");
    assert_eq!(fields[0].id, "field-1");
}

#[tokio::test]
async fn list_fields_for_unknown_base_group_is_not_found() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [],
            "included": {"baseCustomFieldGroups": [], "customFields": []}
        })))
        .mount(&server)
        .await;

    let result = client_for(&server).list_fields("missing", true).await;

    assert!(
        matches!(result, Err(PlankaError::NotFound { .. })),
        "an absent base group must be a not-found, not an empty list; got {result:?}"
    );
}

/// Base groups have their own PATCH and DELETE routes. `PATCH
/// /api/custom-field-groups/{baseId}` returns a real 404.
#[tokio::test]
async fn base_group_update_and_delete_use_base_routes() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/base-custom-field-groups/base-1"))
        .and(body_json(json!({"name": "Docs"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": "base-1",
                "name": "Docs",
                "projectId": "project-1",
                "createdAt": "2026-08-10T00:00:00Z",
                "updatedAt": "2026-08-10T01:00:00Z"
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/api/base-custom-field-groups/base-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": "base-1",
                "name": "Docs",
                "projectId": "project-1",
                "createdAt": "2026-08-10T00:00:00Z",
                "updatedAt": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server);
    let updated = client
        .update_base_field_group(
            "base-1",
            UpdateCustomFieldGroup {
                name: Some("Docs".to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "Docs");

    client.delete_base_field_group("base-1").await.unwrap();
}

/// "Leave unchanged" and "set false" must stay distinguishable on the wire.
#[tokio::test]
async fn update_field_sends_explicit_false_for_show_on_front() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/custom-fields/field-1"))
        .and(body_json(json!({"showOnFrontOfCard": false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": "field-1",
                "name": "Specification",
                "position": 65536.0,
                "showOnFrontOfCard": false,
                "customFieldGroupId": "group-1",
                "baseCustomFieldGroupId": null,
                "createdAt": "2026-08-10T00:00:00Z",
                "updatedAt": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let field = client_for(&server)
        .update_field(
            "field-1",
            UpdateCustomField {
                name: None,
                show_on_front_of_card: Some(false),
            },
        )
        .await
        .unwrap();

    assert!(!field.show_on_front_of_card);
}
