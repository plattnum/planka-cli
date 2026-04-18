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

fn card_json(id: &str, list_id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "listId": list_id,
        "boardId": "board-1",
        "name": name,
        "description": null,
        "position": 65536.0,
        "isSubscribed": false,
        "createdAt": "2026-04-14T12:00:00Z",
        "updatedAt": null
    })
}

fn board_snapshot(
    labels: serde_json::Value,
    cards: serde_json::Value,
    card_labels: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "item": {
            "id": "board-1",
            "name": "Work",
            "position": 65536.0,
            "projectId": "project-1",
            "createdAt": "2026-04-14T12:00:00Z",
            "updatedAt": null
        },
        "included": {
            "lists": [],
            "cards": cards,
            "labels": labels,
            "cardLabels": card_labels,
            "boardMemberships": []
        }
    })
}

#[tokio::test]
async fn card_list_by_list_and_label_id_filters_cards() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/lists/list-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "list-1",
                "boardId": "board-1",
                "name": "Backlog",
                "position": 65536.0,
                "color": null,
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": null
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot(
            serde_json::json!([
                {
                    "id": "label-red",
                    "boardId": "board-1",
                    "name": "Urgent",
                    "color": "berry-red",
                    "position": 65536.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]),
            serde_json::json!([
                card_json("card-1", "list-1", "Fix auth"),
                card_json("card-2", "list-1", "Write docs"),
                card_json("card-3", "list-2", "Other list")
            ]),
            serde_json::json!([
                {"id": "cl-1", "cardId": "card-1", "labelId": "label-red", "createdAt": "2026-04-14T12:00:00Z"},
                {"id": "cl-2", "cardId": "card-3", "labelId": "label-red", "createdAt": "2026-04-14T12:00:00Z"}
            ]),
        )))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card",
            "list",
            "--list",
            "list-1",
            "--label",
            "label-red",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["meta"]["count"], 1);
    assert_eq!(json["data"][0]["id"], "card-1");
}

#[tokio::test]
async fn card_list_by_board_and_label_name_uses_three_tier_match() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot(
            serde_json::json!([
                {
                    "id": "label-red",
                    "boardId": "board-1",
                    "name": "Urgent",
                    "color": "berry-red",
                    "position": 65536.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]),
            serde_json::json!([
                card_json("card-1", "list-1", "Fix auth"),
                card_json("card-2", "list-1", "Write docs")
            ]),
            serde_json::json!([
                {"id": "cl-1", "cardId": "card-1", "labelId": "label-red", "createdAt": "2026-04-14T12:00:00Z"}
            ]),
        )))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card", "list", "--board", "board-1", "--label", "urgent", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["meta"]["count"], 1);
    assert_eq!(json["data"][0]["id"], "card-1");
}

#[tokio::test]
async fn repeated_label_flags_use_and_semantics() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot(
            serde_json::json!([
                {
                    "id": "label-red",
                    "boardId": "board-1",
                    "name": "Urgent",
                    "color": "berry-red",
                    "position": 65536.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                },
                {
                    "id": "label-blue",
                    "boardId": "board-1",
                    "name": "Backend",
                    "color": "ocean-blue",
                    "position": 131072.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]),
            serde_json::json!([
                card_json("card-1", "list-1", "Fix auth"),
                card_json("card-2", "list-1", "Write docs"),
                card_json("card-3", "list-1", "Clean up")
            ]),
            serde_json::json!([
                {"id": "cl-1", "cardId": "card-1", "labelId": "label-red", "createdAt": "2026-04-14T12:00:00Z"},
                {"id": "cl-2", "cardId": "card-1", "labelId": "label-blue", "createdAt": "2026-04-14T12:00:00Z"},
                {"id": "cl-3", "cardId": "card-2", "labelId": "label-red", "createdAt": "2026-04-14T12:00:00Z"},
                {"id": "cl-4", "cardId": "card-3", "labelId": "label-blue", "createdAt": "2026-04-14T12:00:00Z"}
            ]),
        )))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card", "list", "--board", "board-1", "--label", "Urgent", "--label", "Backend",
            "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["meta"]["count"], 1);
    assert_eq!(json["data"][0]["id"], "card-1");
}

#[tokio::test]
async fn ambiguous_label_name_exits_2_and_lists_candidates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot(
            serde_json::json!([
                {
                    "id": "label-1",
                    "boardId": "board-1",
                    "name": "Bug",
                    "color": "berry-red",
                    "position": 65536.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                },
                {
                    "id": "label-2",
                    "boardId": "board-1",
                    "name": "Bugfix",
                    "color": "ocean-blue",
                    "position": 131072.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]),
            serde_json::json!([]),
            serde_json::json!([]),
        )))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args(["card", "list", "--board", "board-1", "--label", "ug"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains(
            "matched multiple labels on this board",
        ))
        .stderr(predicate::str::contains("use a label ID"))
        .stderr(predicate::str::contains("Bug (label-1)"))
        .stderr(predicate::str::contains("Bugfix (label-2)"));
}

#[tokio::test]
async fn unknown_label_exits_4() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot(
            serde_json::json!([]),
            serde_json::json!([]),
            serde_json::json!([]),
        )))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card", "list", "--board", "board-1", "--label", "missing", "--output", "json",
        ])
        .assert()
        .failure()
        .code(4)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["error"]["type"], "ResourceNotFound");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("No label matching 'missing' was found on this board")
    );
}

#[tokio::test]
async fn label_from_another_board_exits_4() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/lists/list-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "item": {
                "id": "list-1",
                "boardId": "board-1",
                "name": "Backlog",
                "position": 65536.0,
                "color": null,
                "createdAt": "2026-04-14T12:00:00Z",
                "updatedAt": null
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot(
            serde_json::json!([
                {
                    "id": "label-red",
                    "boardId": "board-1",
                    "name": "Urgent",
                    "color": "berry-red",
                    "position": 65536.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]),
            serde_json::json!([]),
            serde_json::json!([]),
        )))
        .mount(&server)
        .await;

    plnk_with_server(&server.uri())
        .args([
            "card",
            "list",
            "--list",
            "list-1",
            "--label",
            "label-from-other-board",
            "--output",
            "json",
        ])
        .assert()
        .failure()
        .code(4);
}

#[tokio::test]
async fn card_find_by_board_and_label_works_without_title() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot(
            serde_json::json!([
                {
                    "id": "label-red",
                    "boardId": "board-1",
                    "name": "Urgent",
                    "color": "berry-red",
                    "position": 65536.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]),
            serde_json::json!([
                card_json("card-1", "list-1", "Fix auth"),
                card_json("card-2", "list-1", "Write docs")
            ]),
            serde_json::json!([
                {"id": "cl-1", "cardId": "card-1", "labelId": "label-red", "createdAt": "2026-04-14T12:00:00Z"}
            ]),
        )))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card", "find", "--board", "board-1", "--label", "Urgent", "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["meta"]["count"], 1);
    assert_eq!(json["data"][0]["id"], "card-1");
}

#[tokio::test]
async fn card_find_by_board_and_label_and_title_combines_predicates() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/boards/board-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(board_snapshot(
            serde_json::json!([
                {
                    "id": "label-red",
                    "boardId": "board-1",
                    "name": "Urgent",
                    "color": "berry-red",
                    "position": 65536.0,
                    "createdAt": "2026-04-14T12:00:00Z",
                    "updatedAt": null
                }
            ]),
            serde_json::json!([
                card_json("card-1", "list-1", "Fix auth"),
                card_json("card-2", "list-1", "Fix docs"),
                card_json("card-3", "list-1", "Other")
            ]),
            serde_json::json!([
                {"id": "cl-1", "cardId": "card-1", "labelId": "label-red", "createdAt": "2026-04-14T12:00:00Z"},
                {"id": "cl-2", "cardId": "card-3", "labelId": "label-red", "createdAt": "2026-04-14T12:00:00Z"}
            ]),
        )))
        .mount(&server)
        .await;

    let output = plnk_with_server(&server.uri())
        .args([
            "card", "find", "--board", "board-1", "--label", "Urgent", "--title", "Fix auth",
            "--output", "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["meta"]["count"], 1);
    assert_eq!(json["data"][0]["id"], "card-1");
}
