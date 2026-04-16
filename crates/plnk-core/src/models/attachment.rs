use serde::{Deserialize, Serialize};

use super::ResourceId;

/// Attachment metadata for a file uploaded to a card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: ResourceId,
    pub card_id: ResourceId,
    pub name: String,
    #[serde(default)]
    pub data: Option<AttachmentData>,
    #[serde(default)]
    pub creator_user_id: Option<ResourceId>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Nested data object inside an attachment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentData {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub mime_type: Option<String>,
}
