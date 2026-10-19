use serde::{Deserialize, Serialize};

use super::ResourceId;
use super::common::null_as_default;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct List {
    pub id: ResourceId,
    pub board_id: ResourceId,
    #[serde(default, deserialize_with = "null_as_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub position: f64,
    /// `active` or `closed` for user lists; `archive` / `trash` for the board's
    /// system lists. Moving a card into a `closed` list marks it closed.
    #[serde(rename = "type", default, deserialize_with = "null_as_default")]
    pub list_type: String,
    #[serde(default)]
    pub color: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// List types a client may create or set. `archive` and `trash` exist on
/// every board but are system lists; the server rejects them here.
pub const LIST_TYPES: [&str; 2] = ["active", "closed"];

/// The named colors Planka accepts for a list.
pub const LIST_COLORS: [&str; 10] = [
    "berry-red",
    "pumpkin-orange",
    "lagoon-blue",
    "pink-tulip",
    "light-mud",
    "orange-peel",
    "bright-moss",
    "antique-blue",
    "dark-granite",
    "turquoise-sea",
];

/// Parameters for creating a list.
///
/// `POST /api/boards/{id}/lists` silently drops `color`, so there is no color
/// field here: set it with a follow-up [`UpdateList`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateList {
    pub board_id: ResourceId,
    pub name: String,
    /// Required by Planka: one of [`LIST_TYPES`].
    #[serde(rename = "type")]
    pub list_type: String,
    pub position: f64,
}

/// Parameters for updating a list. Unset fields are not sent.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<f64>,
    /// One of [`LIST_TYPES`].
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub list_type: Option<String>,
    /// `Some(Some(color))` sets a color, `Some(None)` sends `null` to clear it,
    /// `None` leaves it unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Option<String>>,
}
