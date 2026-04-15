use serde::{Deserialize, Serialize};

use super::ResourceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: ResourceId,
    pub project_id: ResourceId,
    pub name: String,
    pub position: f64,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for creating a board.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBoard {
    pub project_id: ResourceId,
    pub name: String,
    /// Planka requires `"type": "kanban"` for board creation.
    #[serde(rename = "type")]
    pub board_type: String,
    pub position: f64,
}

/// Parameters for updating a board.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBoard {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
