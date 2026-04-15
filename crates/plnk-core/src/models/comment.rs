use serde::{Deserialize, Serialize};

use super::ResourceId;

/// A comment on a card (Planka calls these "comment actions").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: ResourceId,
    pub card_id: ResourceId,
    pub user_id: ResourceId,
    pub text: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for creating a comment.
#[derive(Debug, Clone, Serialize)]
pub struct CreateComment {
    pub text: String,
}

/// Parameters for updating a comment.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateComment {
    pub text: String,
}
