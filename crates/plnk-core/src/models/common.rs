use serde::{Deserialize, Serialize};

/// Opaque string identifier. Supports UUIDs, numeric IDs, or any
/// future format Planka might use.
pub type ResourceId = String;

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

/// Structured error detail inside the error envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}
