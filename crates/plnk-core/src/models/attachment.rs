use serde::{Deserialize, Serialize};

use super::ResourceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: ResourceId,
    pub card_id: ResourceId,
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}
