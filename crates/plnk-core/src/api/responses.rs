//! Private API response types matching Planka's wire format.
//!
//! These structs handle Planka's envelope patterns:
//! - Single resource: `{ "item": T, "included": {...} }`
//! - Collection: `{ "items": [T, ...], "included": {...} }`
//!
//! Domain models deserialize directly since they already match the wire format.
//! These envelopes just unwrap the outer structure.

use serde::Deserialize;

/// Planka's single-resource response envelope.
/// Used by GET /api/<resource>/{id}, POST, PATCH.
#[derive(Debug, Deserialize)]
pub(crate) struct ItemResponse<T> {
    pub item: T,
}

/// Planka's collection response envelope.
/// Used by GET /api/<resource> list endpoints.
#[derive(Debug, Deserialize)]
pub(crate) struct ItemsResponse<T> {
    pub items: Vec<T>,
}

/// Board snapshot response — GET /api/boards/{id} returns nested included data.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoardSnapshot {
    pub item: BoardSnapshotItem,
    pub included: BoardSnapshotIncluded,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoardSnapshotItem {
    pub id: String,
    pub name: String,
    pub position: f64,
    pub project_id: String,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoardSnapshotIncluded {
    #[serde(default)]
    pub lists: Vec<crate::models::List>,
    #[serde(default)]
    pub cards: Vec<crate::models::Card>,
}

/// Project snapshot response — GET /api/projects/{id} returns nested included data.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSnapshot {
    pub item: crate::models::Project,
    pub included: ProjectSnapshotIncluded,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSnapshotIncluded {
    #[serde(default)]
    pub boards: Vec<crate::models::Board>,
}

/// Cards list response — GET /api/lists/{id}/cards returns items + included.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CardsListResponse {
    pub items: Vec<crate::models::Card>,
}
