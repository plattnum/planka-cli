//! Wire-level tests for `TaskListApi` and the checklist-aware `TaskApi`.
//!
//! The load-bearing assertion is that `create_task` posts to the task list
//! it was given and never goes looking for one on the card. The SDK used to
//! pick the first checklist it saw, which silently misplaced tasks on any
//! card with more than one.

use plnk_core::api::{PlankaClientV1, TaskApi, TaskListApi};
use plnk_core::client::HttpClient;
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateTaskList, UpdateTaskList};
use serde_json::json;
use url::Url;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer) -> PlankaClientV1 {
    let base_url = Url::parse(&server.uri()).unwrap();
    let http = HttpClient::new(base_url, "test-api-key").unwrap();
    PlankaClientV1::new(http)
}

fn task_list_json(id: &str, name: &str, position: f64) -> serde_json::Value {
    json!({
        "id": id,
        "cardId": "card-1",
        "name": name,
        "position": position,
        "showOnFrontOfCard": true,
        "hideCompletedTasks": false,
        "createdAt": "2026-09-12T00:00:00Z",
        "updatedAt": null
    })
}

fn task_json(id: &str, task_list_id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "taskListId": task_list_id,
        "name": "Write tests",
        "isCompleted": false,
        "position": 65536.0,
        "linkedCardId": null,
        "assigneeUserId": null,
        "createdAt": "2026-09-12T00:00:00Z",
        "updatedAt": null
    })
}

/// Captured from `GET /api/task-lists/{id}` on 2026-09-12: `item` plus
/// `included.tasks`.
fn task_list_snapshot() -> serde_json::Value {
    json!({
        "item": task_list_json("tl-1", "Tasks", 65536.0),
        "included": { "tasks": [task_json("task-1", "tl-1")] }
    })
}

#[tokio::test]
async fn create_task_posts_to_the_given_task_list() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/task-lists/tl-2/tasks"))
        .and(body_json(json!({"name": "X", "position": 65536.0})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"item": task_json("task-9", "tl-2")})),
        )
        .expect(1)
        .mount(&server)
        .await;
    // No card lookup may happen. Mount nothing for /api/cards so any such
    // request 404s and fails the call.

    let task = client_for(&server).create_task("tl-2", "X").await.unwrap();

    assert_eq!(task.id, "task-9");
    assert_eq!(task.task_list_id, "tl-2");
}

#[tokio::test]
async fn list_tasks_in_list_reads_included_tasks() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/task-lists/tl-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_list_snapshot()))
        .expect(1)
        .mount(&server)
        .await;

    let tasks = client_for(&server)
        .list_tasks_in_list("tl-1")
        .await
        .unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_list_id, "tl-1");
}

#[tokio::test]
async fn get_task_list_reads_item() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/task-lists/tl-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_list_snapshot()))
        .expect(1)
        .mount(&server)
        .await;

    let tl = client_for(&server).get_task_list("tl-1").await.unwrap();

    assert_eq!(tl.id, "tl-1");
    assert_eq!(tl.name, "Tasks");
    assert!(tl.show_on_front_of_card);
}

#[tokio::test]
async fn create_task_list_omits_unset_flags() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/cards/card-1/task-lists"))
        .and(body_json(json!({"name": "QA", "position": 65536.0})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"item": task_list_json("tl-3", "QA", 65536.0)})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let tl = client_for(&server)
        .create_task_list(
            "card-1",
            CreateTaskList {
                name: "QA".to_string(),
                position: 65536.0,
                show_on_front_of_card: None,
                hide_completed_tasks: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(tl.id, "tl-3");
}

#[tokio::test]
async fn update_task_list_patches_only_set_fields() {
    let server = MockServer::start().await;

    Mock::given(method("PATCH"))
        .and(path("/api/task-lists/tl-1"))
        .and(body_json(json!({"hideCompletedTasks": true})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"item": task_list_json("tl-1", "Tasks", 65536.0)})),
        )
        .expect(1)
        .mount(&server)
        .await;

    client_for(&server)
        .update_task_list(
            "tl-1",
            UpdateTaskList {
                hide_completed_tasks: Some(true),
                ..UpdateTaskList::default()
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn list_task_lists_is_sorted_by_position() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": "card-1", "listId": "list-1", "boardId": "board-1", "name": "A card",
                "description": null, "position": 65536.0, "dueDate": null,
                "isDueCompleted": null, "isClosed": false, "isSubscribed": false,
                "creatorUserId": null, "createdAt": "2026-09-12T00:00:00Z", "updatedAt": null
            },
            "included": {
                "taskLists": [
                    task_list_json("tl-b", "Second", 131_072.0),
                    task_list_json("tl-a", "First", 65536.0)
                ],
                "tasks": []
            }
        })))
        .mount(&server)
        .await;

    let lists = client_for(&server).list_task_lists("card-1").await.unwrap();

    let ids: Vec<&str> = lists.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, vec!["tl-a", "tl-b"]);
}

#[tokio::test]
async fn find_task_lists_matches_by_name_within_card() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/cards/card-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "item": {
                "id": "card-1", "listId": "list-1", "boardId": "board-1", "name": "A card",
                "description": null, "position": 65536.0, "dueDate": null,
                "isDueCompleted": null, "isClosed": false, "isSubscribed": false,
                "creatorUserId": null, "createdAt": "2026-09-12T00:00:00Z", "updatedAt": null
            },
            "included": {
                "taskLists": [
                    task_list_json("tl-a", "Design", 65536.0),
                    task_list_json("tl-b", "QA", 131_072.0)
                ],
                "tasks": []
            }
        })))
        .mount(&server)
        .await;

    let found = client_for(&server)
        .find_task_lists("card-1", "qa")
        .await
        .unwrap();

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "tl-b");
}

#[tokio::test]
async fn delete_task_list_404_maps_to_remote_404() {
    let server = MockServer::start().await;

    Mock::given(method("DELETE"))
        .and(path("/api/task-lists/tl-missing"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({"code": "E_NOT_FOUND", "message": "Task list not found"})),
        )
        .mount(&server)
        .await;

    let err = client_for(&server)
        .delete_task_list("tl-missing")
        .await
        .unwrap_err();

    assert!(
        matches!(err, PlankaError::Remote404 { .. }),
        "expected Remote404, got {err:?}"
    );
}
