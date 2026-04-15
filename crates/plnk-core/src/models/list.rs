use serde::{Deserialize, Serialize};

use super::ResourceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct List {
    pub id: ResourceId,
    pub board_id: ResourceId,
    pub name: String,
    pub position: f64,
    #[serde(default)]
    pub color: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for creating a list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateList {
    pub board_id: ResourceId,
    pub name: String,
    /// Planka requires `"type": "active"` for list creation.
    #[serde(rename = "type")]
    pub list_type: String,
    pub position: f64,
}

/// Parameters for updating a list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<f64>,
}
