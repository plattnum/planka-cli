use serde::{Deserialize, Serialize};

use super::ResourceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: ResourceId,
    pub board_id: ResourceId,
    pub name: Option<String>,
    pub color: String,
    pub position: f64,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A card-label association.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CardLabel {
    pub id: ResourceId,
    pub card_id: ResourceId,
    pub label_id: ResourceId,
    pub created_at: String,
}

/// Parameters for creating a label.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLabel {
    pub board_id: ResourceId,
    pub name: String,
    pub color: String,
    pub position: f64,
}

/// Parameters for updating a label.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLabel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}
