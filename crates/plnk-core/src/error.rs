use std::fmt::Write as _;

use crate::models::{ErrorDetail, ErrorEnvelope};

/// HTTP methods for which a 404 is ambiguous between "doesn't exist" and
/// "you don't have permission". Planka returns 404 for permission denials
/// on mutation endpoints, so surface the hint to the user.
fn is_write_method(method: &str) -> bool {
    matches!(
        method.to_ascii_uppercase().as_str(),
        "POST" | "PATCH" | "PUT" | "DELETE"
    )
}

/// Display formatter for `Remote404`. Includes the server's own message
/// when present and appends a permission hint for write methods.
fn format_remote_404(method: &str, path: &str, server_message: &str) -> String {
    let mut out = format!("HTTP 404 on {method} {path}");
    if !server_message.is_empty() {
        let _ = write!(out, "\n  Server message: {server_message}");
    }
    if is_write_method(method) {
        out.push_str(
            "\n  Note: Planka returns 404 for permission denials on writes. \
             Verify your account has access to this resource.",
        );
    }
    out
}

/// Unified error type for the plnk-core library.
///
/// Maps to the CLI spec's error types (Section 10) and exit codes.
#[derive(Debug, thiserror::Error)]
pub enum PlankaError {
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Resource not found: {resource_type} {id}")]
    NotFound { resource_type: String, id: String },

    /// HTTP 404 from the Planka server.
    ///
    /// Distinguished from `NotFound` because Planka also uses 404 to signal
    /// permission denials — the caller may not actually lack the resource.
    #[error("{}", format_remote_404(.method, .path, .server_message))]
    Remote404 {
        method: String,
        path: String,
        server_message: String,
    },

    #[error("Missing required option: {field}")]
    MissingRequiredOption { field: String },

    #[error("Invalid option value for {field}: {message}")]
    InvalidOptionValue { field: String, message: String },

    #[error("Mutually exclusive options: {fields:?}")]
    MutuallyExclusiveOptions { fields: Vec<String> },

    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("File read error: {path}: {source}")]
    FileReadError {
        path: String,
        source: std::io::Error,
    },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

impl PlankaError {
    /// Map to CLI exit code per spec Section 10.1.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::MissingRequiredOption { .. }
            | Self::InvalidOptionValue { .. }
            | Self::MutuallyExclusiveOptions { .. }
            | Self::Url(_) => 2,

            Self::AuthenticationFailed { .. } => 3,

            Self::NotFound { .. } | Self::Remote404 { .. } => 4,

            Self::ApiError { .. } | Self::Http(_) => 5,

            Self::FileReadError { .. }
            | Self::Io(_)
            | Self::Json(_)
            | Self::TomlDeserialize(_)
            | Self::TomlSerialize(_) => 1,
        }
    }

    /// Map to spec error type string per Section 10.3.
    pub fn error_type(&self) -> &'static str {
        match self {
            Self::MissingRequiredOption { .. } => "MissingRequiredOption",
            Self::InvalidOptionValue { .. } | Self::Url(_) => "InvalidOptionValue",
            Self::MutuallyExclusiveOptions { .. } => "MutuallyExclusiveOptions",
            Self::NotFound { .. } | Self::Remote404 { .. } => "ResourceNotFound",
            Self::AuthenticationFailed { .. } => "AuthenticationFailed",
            Self::ApiError { .. } | Self::Http(_) => "ApiError",
            Self::FileReadError { .. } | Self::Io(_) => "FileReadError",
            Self::Json(_) | Self::TomlDeserialize(_) | Self::TomlSerialize(_) => {
                "SerializationError"
            }
        }
    }

    /// Render as the spec's structured JSON error envelope.
    #[must_use]
    pub fn to_error_envelope(&self) -> ErrorEnvelope {
        let field = match self {
            Self::MissingRequiredOption { field } | Self::InvalidOptionValue { field, .. } => {
                Some(field.clone())
            }
            _ => None,
        };

        ErrorEnvelope {
            success: false,
            error: ErrorDetail {
                error_type: self.error_type().to_string(),
                message: self.to_string(),
                field,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_validation_errors() {
        let err = PlankaError::MissingRequiredOption {
            field: "--list".to_string(),
        };
        assert_eq!(err.exit_code(), 2);
        assert_eq!(err.error_type(), "MissingRequiredOption");

        let err = PlankaError::InvalidOptionValue {
            field: "--position".to_string(),
            message: "must be a number".to_string(),
        };
        assert_eq!(err.exit_code(), 2);
        assert_eq!(err.error_type(), "InvalidOptionValue");

        let err = PlankaError::MutuallyExclusiveOptions {
            fields: vec![
                "--description".to_string(),
                "--description-file".to_string(),
            ],
        };
        assert_eq!(err.exit_code(), 2);
        assert_eq!(err.error_type(), "MutuallyExclusiveOptions");
    }

    #[test]
    fn exit_code_auth_failure() {
        let err = PlankaError::AuthenticationFailed {
            message: "No credentials found".to_string(),
        };
        assert_eq!(err.exit_code(), 3);
        assert_eq!(err.error_type(), "AuthenticationFailed");
    }

    #[test]
    fn exit_code_not_found() {
        let err = PlankaError::NotFound {
            resource_type: "card".to_string(),
            id: "1234".to_string(),
        };
        assert_eq!(err.exit_code(), 4);
        assert_eq!(err.error_type(), "ResourceNotFound");

        let err = PlankaError::Remote404 {
            method: "GET".to_string(),
            path: "/api/cards/1".to_string(),
            server_message: String::new(),
        };
        assert_eq!(err.exit_code(), 4);
        assert_eq!(err.error_type(), "ResourceNotFound");
    }

    #[test]
    fn remote_404_display_includes_method_path_and_message() {
        let err = PlankaError::Remote404 {
            method: "GET".to_string(),
            path: "/api/cards/1".to_string(),
            server_message: "No such card".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("GET /api/cards/1"),
            "method + path missing: {msg}"
        );
        assert!(
            msg.contains("No such card"),
            "server message missing: {msg}"
        );
        // GET is a read — no permission hint
        assert!(
            !msg.contains("permission"),
            "permission hint leaked onto GET: {msg}"
        );
    }

    #[test]
    fn remote_404_display_warns_about_permissions_on_writes() {
        for method in ["POST", "PATCH", "PUT", "DELETE"] {
            let err = PlankaError::Remote404 {
                method: method.to_string(),
                path: "/api/projects/1/boards".to_string(),
                server_message: String::new(),
            };
            let msg = err.to_string();
            assert!(
                msg.contains("permission"),
                "missing permission hint on {method}: {msg}"
            );
            assert!(msg.contains(method), "method missing: {msg}");
        }
    }

    #[test]
    fn exit_code_api_error() {
        let err = PlankaError::ApiError {
            status: 500,
            message: "Internal Server Error".to_string(),
        };
        assert_eq!(err.exit_code(), 5);
        assert_eq!(err.error_type(), "ApiError");
    }

    #[test]
    fn error_envelope_structure() {
        let err = PlankaError::MissingRequiredOption {
            field: "--list".to_string(),
        };
        let envelope = err.to_error_envelope();
        assert!(!envelope.success);
        assert_eq!(envelope.error.error_type, "MissingRequiredOption");
        assert_eq!(envelope.error.field, Some("--list".to_string()));

        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["success"], false);
        assert_eq!(json["error"]["type"], "MissingRequiredOption");
        assert_eq!(json["error"]["field"], "--list");
    }

    #[test]
    fn error_envelope_no_field() {
        let err = PlankaError::NotFound {
            resource_type: "card".to_string(),
            id: "1234".to_string(),
        };
        let envelope = err.to_error_envelope();
        assert!(envelope.error.field.is_none());

        let json = serde_json::to_value(&envelope).unwrap();
        assert!(json["error"].get("field").is_none());
    }
}
