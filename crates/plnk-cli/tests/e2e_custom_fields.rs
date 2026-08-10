//! End-to-end tests for `plnk card field`.
//!
//! The centrepiece is name resolution through a base group. A card group
//! adopted from a base group has `name: null`, so matching a card group by its
//! own name alone matches nothing for every template-adopted group — the common
//! case. These tests pin that behaviour.

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

/// A card carrying one group adopted from a base group — so its own `name` is
/// null and its `customFields` are empty, exactly as the live server returns.
fn card_snapshot() -> serde_json::Value {
    serde_json::json!({
        "item": {
            "id": "card-1",
            "listId": "list-1",
            "boardId": "board-1",
            "name": "A card",
            "description": null,
            "position": 65536.0,
            "isSubscribed": false,
            "createdAt": "2026-08-10T00:00:00Z",
            "updatedAt": null
        },
        "included": {
            "taskLists": [],
            "tasks": [],
            "cardLabels": [],
            "cardMemberships": [],
            "attachments": [],
            "customFieldGroups": [
                {
                    "id": "card-group-1",
                    "name": null,
                    "boardId": null,
                    "cardId": "card-1",
                    "baseCustomFieldGroupId": "base-1",
                    "position": 65536.0,
                    "createdAt": "2026-08-10T00:00:00Z",
                    "updatedAt": null
                }
            ],
            "customFields": [],
            "customFieldValues": []
        }
    })
}

/// The projects list — the only endpoint exposing base groups and their fields.
fn projects_list(extra_fields: &serde_json::Value) -> serde_json::Value {
    let mut fields = vec![serde_json::json!({
        "id": "field-1",
        "name": "Specification",
        "position": 65536.0,
        "showOnFrontOfCard": true,
        "customFieldGroupId": null,
        "baseCustomFieldGroupId": "base-1",
        "createdAt": "2026-08-10T00:00:00Z",
        "updatedAt": null
    })];
    if let Some(more) = extra_fields.as_array() {
        fields.extend(more.iter().cloned());
    }

    serde_json::json!({
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
            "customFields": fields
        }
    })
}

/// The adopted group's own snapshot is empty — its fields live on the base
/// group. This is what makes the base-group fallback necessary for fields too,
/// not just for the group name.
fn empty_group_snapshot() -> serde_json::Value {
    serde_json::json!({
        "item": {
            "id": "card-group-1",
            "name": null,
            "boardId": null,
            "cardId": "card-1",
            "baseCustomFieldGroupId": "base-1",
            "position": 65536.0,
            "createdAt": "2026-08-10T00:00:00Z",
            "updatedAt": null
        },
        "included": {"customFields": []}
    })
}

fn value_response() -> serde_json::Value {
    serde_json::json!({
        "item": {
            "id": "value-1",
            "cardId": "card-1",
            "customFieldGroupId": "card-group-1",
            "customFieldId": "field-1",
            "content": "specs/x.html",
            "createdAt": "2026-08-10T00:00:00Z",
            "updatedAt": null
        }
    })
}

async fn mount_resolution_fixtures(server: &MockServer, extra_fields: &serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(card_snapshot()))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(projects_list(extra_fields)))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/custom-field-groups/card-group-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_group_snapshot()))
        .mount(server)
        .await;
}

/// The single most likely bug in the feature: `--group Documentation` must
/// match a card group whose own name is null, via its base group.
#[tokio::test]
async fn set_resolves_group_name_through_the_base_group() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(&server, &serde_json::json!([])).await;

    Mock::given(method("PATCH"))
        .and(path(
            "/api/cards/card-1/custom-field-values/customFieldGroupId:card-group-1:customFieldId:field-1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(value_response()))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "Specification",
            "--value",
            "specs/x.html",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("specs/x.html"));
}

/// The field name must also resolve through the base group, since the adopted
/// group carries no fields of its own.
#[tokio::test]
async fn clear_resolves_names_through_the_base_group() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(&server, &serde_json::json!([])).await;

    Mock::given(method("DELETE"))
        .and(path(
            "/api/cards/card-1/custom-field-values/customFieldGroupId:card-group-1:customFieldId:field-1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(value_response()))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "clear",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "Specification",
        ])
        .assert()
        .success();
}

/// Clearing an already-unset value satisfies the intent, so it exits 0.
#[tokio::test]
async fn clear_on_an_unset_value_succeeds() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(&server, &serde_json::json!([])).await;

    Mock::given(method("DELETE"))
        .and(path(
            "/api/cards/card-1/custom-field-values/customFieldGroupId:card-group-1:customFieldId:field-1",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "code": "E_NOT_FOUND",
            "message": "Custom field value not found"
        })))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "clear",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "Specification",
        ])
        .assert()
        .success();
}

/// Passing IDs bypasses resolution entirely — no name lookups are needed, so
/// this works against a card whose groups have no names at all.
#[tokio::test]
async fn set_by_id_bypasses_resolution() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(&server, &serde_json::json!([])).await;

    Mock::given(method("PATCH"))
        .and(path(
            "/api/cards/card-1/custom-field-values/customFieldGroupId:card-group-1:customFieldId:field-1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(value_response()))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "card-group-1",
            "--field",
            "field-1",
            "--value",
            "specs/x.html",
        ])
        .assert()
        .success();
}

/// An over-long value must fail before any request is issued. No HTTP mocks are
/// mounted at all, so any request would fail the test.
#[tokio::test]
async fn over_long_value_exits_2_without_issuing_a_request() {
    let server = MockServer::start().await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "Specification",
            "--value",
            &"a".repeat(513),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("512"));

    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "an over-long value must not reach the server"
    );
}

/// A 512-character value is accepted — the boundary is inclusive.
#[tokio::test]
async fn value_at_the_limit_is_accepted() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(&server, &serde_json::json!([])).await;

    Mock::given(method("PATCH"))
        .and(path(
            "/api/cards/card-1/custom-field-values/customFieldGroupId:card-group-1:customFieldId:field-1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(value_response()))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "Specification",
            "--value",
            &"a".repeat(512),
        ])
        .assert()
        .success();
}

/// The server rejects an empty string outright, so there is no "set to empty"
/// behaviour to expose — the message must point at `card field clear`.
#[tokio::test]
async fn empty_value_exits_2_and_names_clear() {
    let server = MockServer::start().await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "Specification",
            "--value",
            "",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("card field clear"));

    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "an empty value must not reach the server"
    );
}

#[tokio::test]
async fn unknown_group_name_exits_4() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(&server, &serde_json::json!([])).await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "NoSuchGroup",
            "--field",
            "Specification",
            "--value",
            "x",
        ])
        .assert()
        .code(4);
}

#[tokio::test]
async fn unknown_field_name_exits_4() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(&server, &serde_json::json!([])).await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "NoSuchField",
            "--value",
            "x",
        ])
        .assert()
        .code(4);
}

/// An ambiguous name must exit 2 and name every candidate, so the caller can
/// pick an ID.
#[tokio::test]
async fn ambiguous_field_name_exits_2_and_lists_candidates() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(
        &server,
        &serde_json::json!([{
            "id": "field-2",
            "name": "Speculative",
            "position": 131_072.0,
            "showOnFrontOfCard": false,
            "customFieldGroupId": null,
            "baseCustomFieldGroupId": "base-1",
            "createdAt": "2026-08-10T00:00:00Z",
            "updatedAt": null
        }]),
    )
    .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "Spec",
            "--value",
            "x",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Specification"))
        .stderr(predicate::str::contains("Speculative"))
        .stderr(predicate::str::contains("field-1"))
        .stderr(predicate::str::contains("field-2"));
}

/// An exact match must win over a substring match rather than being reported
/// as ambiguous.
#[tokio::test]
async fn exact_name_match_beats_substring_match() {
    let server = MockServer::start().await;
    mount_resolution_fixtures(
        &server,
        &serde_json::json!([{
            "id": "field-2",
            "name": "Specification Notes",
            "position": 131_072.0,
            "showOnFrontOfCard": false,
            "customFieldGroupId": null,
            "baseCustomFieldGroupId": "base-1",
            "createdAt": "2026-08-10T00:00:00Z",
            "updatedAt": null
        }]),
    )
    .await;

    Mock::given(method("PATCH"))
        .and(path(
            "/api/cards/card-1/custom-field-values/customFieldGroupId:card-group-1:customFieldId:field-1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(value_response()))
        .expect(1)
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "field",
            "set",
            "card-1",
            "--group",
            "Documentation",
            "--field",
            "Specification",
            "--value",
            "specs/x.html",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn list_values_renders_in_every_format() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "card-1",
                "listId": "list-1",
                "boardId": "board-1",
                "name": "A card",
                "description": null,
                "position": 65536.0,
                "isSubscribed": false,
                "createdAt": "2026-08-10T00:00:00Z",
                "updatedAt": null
            },
            "included": {
                "taskLists": [], "tasks": [], "cardLabels": [],
                "cardMemberships": [], "attachments": [],
                "customFieldGroups": [], "customFields": [],
                "customFieldValues": [{
                    "id": "value-1",
                    "cardId": "card-1",
                    "customFieldGroupId": "card-group-1",
                    "customFieldId": "field-1",
                    "content": "specs/x.html",
                    "createdAt": "2026-08-10T00:00:00Z",
                    "updatedAt": null
                }]
            }
        })))
        .mount(&server)
        .await;

    for format in ["table", "json", "markdown"] {
        plnk_with_server(&server.uri())
            .args(["card", "field", "list", "card-1", "--output", format])
            .assert()
            .success()
            .stdout(predicate::str::contains("specs/x.html"));
    }
}
