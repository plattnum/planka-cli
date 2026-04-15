use crate::models::{ErrorDetail, ErrorEnvelope};

/// Unified error type for the plnk-core library.
///
/// Maps to the CLI spec's error types (Section 10) and exit codes.
#[derive(Debug, thiserror::Error)]
pub enum PlankaError {
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Resource not found: {resource_type} {id}")]
    NotFound { resource_type: String, id: String },

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

            Self::NotFound { .. } => 4,

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
            Self::NotFound { .. } => "ResourceNotFound",
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
