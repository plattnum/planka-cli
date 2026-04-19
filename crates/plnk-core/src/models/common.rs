use serde::{Deserialize, Deserializer, Serialize};

/// Opaque string identifier. Supports UUIDs, numeric IDs, or any
/// future format Planka might use.
pub type ResourceId = String;

/// Deserialize null JSON values as `T::default()`.
///
/// Planka returns null for some fields on system/archive lists.
/// This deserializer treats null the same as missing, giving the default value.
pub fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}

/// Card/task insertion position.
#[derive(Debug, Clone, PartialEq)]
pub enum Position {
    Top,
    Bottom,
    Index(f64),
}

/// Scope for `find` operations — enforces bounded search at the type level.
#[derive(Debug, Clone, PartialEq)]
pub enum FindScope {
    List(ResourceId),
    Board(ResourceId),
    Project(ResourceId),
}

/// JSON output envelope for successful responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope<T> {
    pub success: bool,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Metadata for collection responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Meta {
    pub count: usize,
}

/// JSON output envelope for error responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorEnvelope {
    pub success: bool,
    pub error: ErrorDetail,
}

/// Structured per-item failure detail inside batch error envelopes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorFailure {
    pub id: ResourceId,
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Structured error detail inside the error envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_ids: Option<Vec<ResourceId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failures: Option<Vec<ErrorFailure>>,
}
