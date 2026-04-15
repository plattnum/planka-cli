use serde::{Deserialize, Serialize};

use super::ResourceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ResourceId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Parameters for creating a project.
#[derive(Debug, Clone, Serialize)]
pub struct CreateProject {
    pub name: String,
}

/// Parameters for updating a project.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
