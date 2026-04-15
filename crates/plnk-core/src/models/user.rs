use serde::{Deserialize, Serialize};

use super::ResourceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: ResourceId,
    pub name: String,
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    pub role: String,
    #[serde(default)]
    pub is_deactivated: bool,
    #[serde(default)]
    pub organization: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}
