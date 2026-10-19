use serde::{Deserialize, Serialize};

use super::ResourceId;

fn default_true() -> bool {
    true
}

/// A task list — groups tasks within a card.
/// Planka uses task lists as an intermediate container.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskList {
    pub id: ResourceId,
    pub card_id: ResourceId,
    pub name: String,
    pub position: f64,
    /// Whether the list is shown on the card face in the web UI.
    /// Planka defaults this to `true` server-side; a payload that omits it
    /// (older captures) is read the same way.
    #[serde(default = "default_true")]
    pub show_on_front_of_card: bool,
    /// Whether completed tasks are hidden in the web UI. Planka defaults to `false`.
    #[serde(default)]
    pub hide_completed_tasks: bool,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A task (checklist item) within a task list.
/// Wire format uses `taskListId`, not `cardId`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: ResourceId,
    pub task_list_id: ResourceId,
    pub name: String,
    pub is_completed: bool,
    pub position: f64,
    #[serde(default)]
    pub linked_card_id: Option<ResourceId>,
    #[serde(default)]
    pub assignee_user_id: Option<ResourceId>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for creating a task.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTask {
    pub name: String,
    pub position: f64,
}

/// Parameters for updating a task.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_completed: Option<bool>,
}

/// Parameters for creating a task list.
///
/// The two display flags are omitted from the request when `None`, so the
/// server applies its own defaults (`showOnFrontOfCard: true`,
/// `hideCompletedTasks: false`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskList {
    pub name: String,
    pub position: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_on_front_of_card: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_completed_tasks: Option<bool>,
}

/// Parameters for updating a task list. Every field is optional; unset
/// fields are not sent.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_on_front_of_card: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_completed_tasks: Option<bool>,
}
