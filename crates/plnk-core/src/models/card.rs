use serde::{Deserialize, Serialize};

use super::ResourceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: ResourceId,
    pub list_id: ResourceId,
    pub board_id: ResourceId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub position: f64,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub is_due_completed: Option<bool>,
    #[serde(default)]
    pub is_closed: bool,
    #[serde(default)]
    pub is_subscribed: bool,
    #[serde(default)]
    pub creator_user_id: Option<ResourceId>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for creating a card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCard {
    pub list_id: ResourceId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Planka requires `"type": "project"` for card creation.
    #[serde(rename = "type")]
    pub card_type: String,
    pub position: f64,
}

/// Parameters for updating a card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCard {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_closed: Option<bool>,
}

/// Parameters for moving a card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCard {
    pub list_id: ResourceId,
    pub position: f64,
}
