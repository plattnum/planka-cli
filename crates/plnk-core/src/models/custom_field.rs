use serde::{Deserialize, Serialize};

use super::ResourceId;

/// A reusable custom field group template attached to a project.
///
/// Base groups carry no `position` — the create request accepts one but the
/// response omits it, so the field is deliberately absent from this model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BaseCustomFieldGroup {
    pub id: ResourceId,
    pub project_id: ResourceId,
    pub name: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A custom field group attached to a board or a card.
///
/// `name` is `None` when the group was adopted from a base group — the display
/// name then lives on the base group referenced by `base_custom_field_group_id`.
/// `board_id` and `card_id` are mutually exclusive in practice, as are
/// `name` and `base_custom_field_group_id`; neither exclusivity is encoded in
/// the type, matching how the wire format represents them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomFieldGroup {
    pub id: ResourceId,
    pub name: Option<String>,
    #[serde(default)]
    pub board_id: Option<ResourceId>,
    #[serde(default)]
    pub card_id: Option<ResourceId>,
    #[serde(default)]
    pub base_custom_field_group_id: Option<ResourceId>,
    pub position: f64,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A named slot inside a custom field group.
///
/// `custom_field_group_id` and `base_custom_field_group_id` are mutually
/// exclusive: a field belongs either to a board/card group or to a base group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomField {
    pub id: ResourceId,
    pub name: String,
    pub position: f64,
    pub show_on_front_of_card: bool,
    #[serde(default)]
    pub custom_field_group_id: Option<ResourceId>,
    #[serde(default)]
    pub base_custom_field_group_id: Option<ResourceId>,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A value stored against a card for one (group, field) pair.
///
/// Planka stores every value as a string; `content` is capped at 512
/// characters server-side and may not be empty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomFieldValue {
    pub id: ResourceId,
    pub card_id: ResourceId,
    pub custom_field_group_id: ResourceId,
    pub custom_field_id: ResourceId,
    pub content: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Maximum length of a custom field value, enforced server-side.
///
/// Verified against a live server: 512 characters returns `200`, 513 returns
/// `400`. Validated client-side so callers get a validation error instead of a
/// wasted round trip.
pub const CUSTOM_FIELD_VALUE_MAX_LEN: usize = 512;

/// Parameters for updating a custom field group.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCustomFieldGroup {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Parameters for updating a custom field.
///
/// `show_on_front_of_card` is `Option<bool>` so that "leave unchanged" and
/// "set to false" stay distinguishable in a PATCH body.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCustomField {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_on_front_of_card: Option<bool>,
}
