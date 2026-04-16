use serde::{Deserialize, Serialize};

use super::ResourceId;

/// Board membership — links a user to a board with a role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoardMembership {
    pub id: ResourceId,
    pub board_id: ResourceId,
    pub user_id: ResourceId,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub can_comment: Option<bool>,
    #[serde(default)]
    pub project_id: Option<ResourceId>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Card membership — links a user (assignee) to a card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CardMembership {
    pub id: ResourceId,
    pub card_id: ResourceId,
    pub user_id: ResourceId,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Project manager — links a user to a project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManager {
    pub id: ResourceId,
    pub project_id: ResourceId,
    pub user_id: ResourceId,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for adding a board membership.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBoardMembership {
    pub user_id: ResourceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Parameters for adding a card membership (assignee).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCardMembership {
    pub user_id: ResourceId,
}

/// Parameters for adding a project manager.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectManager {
    pub user_id: ResourceId,
}
