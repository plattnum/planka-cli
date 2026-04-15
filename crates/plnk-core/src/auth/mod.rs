mod config;
mod login;

pub use config::{ConfigFile, config_path, delete_config, read_config, write_config};
pub use login::{login, validate_token};

use tracing::debug;
use url::Url;

use crate::error::PlankaError;

/// Resolved credentials ready for API use.
#[derive(Debug, Clone)]
pub struct ResolvedCredentials {
    pub server: Url,
    pub token: String,
    pub source: CredentialSource,
}

/// Where the credentials came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    Flags,
    Environment,
    ConfigFile,
}

impl std::fmt::Display for CredentialSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flags => write!(f, "CLI flags"),
            Self::Environment => write!(f, "environment variables"),
            Self::ConfigFile => write!(f, "config file"),
        }
    }
}

/// Resolve credentials using the spec's precedence chain:
/// 1. CLI flags (`--server` + `--token`)
/// 2. Environment variables (`PLANKA_SERVER` + `PLANKA_TOKEN`)
/// 3. Config file (`~/.config/planka/config.toml`)
///
/// # Errors
/// Returns `PlankaError::AuthenticationFailed` if no credentials can be resolved.
pub fn resolve_credentials(
    flag_server: Option<&str>,
    flag_token: Option<&str>,
) -> Result<ResolvedCredentials, PlankaError> {
    // 1. CLI flags
    if let (Some(server), Some(token)) = (flag_server, flag_token) {
        debug!("Credentials resolved from CLI flags");
        let server = Url::parse(server).map_err(|e| PlankaError::InvalidOptionValue {
            field: "--server".to_string(),
            message: format!("Invalid URL: {e}"),
        })?;
        return Ok(ResolvedCredentials {
            server,
            token: token.to_string(),
            source: CredentialSource::Flags,
        });
    }

    // Partial flags = error
    if flag_server.is_some() || flag_token.is_some() {
        return Err(PlankaError::AuthenticationFailed {
            message: "Both --server and --token must be provided together.".to_string(),
        });
    }

    // 2. Environment variables
    let env_server = std::env::var("PLANKA_SERVER").ok();
    let env_token = std::env::var("PLANKA_TOKEN").ok();

    if let (Some(server), Some(token)) = (env_server.as_deref(), env_token.as_deref()) {
        if !server.is_empty() && !token.is_empty() {
            debug!("Credentials resolved from environment variables");
            let server = Url::parse(server).map_err(|e| PlankaError::InvalidOptionValue {
                field: "PLANKA_SERVER".to_string(),
                message: format!("Invalid URL: {e}"),
            })?;
            return Ok(ResolvedCredentials {
                server,
                token: token.to_string(),
                source: CredentialSource::Environment,
            });
        }
    }

    // 3. Config file
    if let Some(config) = read_config()? {
        debug!("Credentials resolved from config file");
        let server = Url::parse(&config.server).map_err(|e| PlankaError::InvalidOptionValue {
            field: "config.server".to_string(),
            message: format!("Invalid URL in config file: {e}"),
        })?;
        return Ok(ResolvedCredentials {
            server,
            token: config.token,
            source: CredentialSource::ConfigFile,
        });
    }

    Err(PlankaError::AuthenticationFailed {
        message: "No credentials found. Run 'plnk auth login', set PLANKA_SERVER/PLANKA_TOKEN, \
                  or pass --server and --token."
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_from_flags() {
        let result =
            resolve_credentials(Some("http://localhost:3000"), Some("test-token")).unwrap();
        assert_eq!(result.source, CredentialSource::Flags);
        assert_eq!(result.token, "test-token");
        assert_eq!(result.server.as_str(), "http://localhost:3000/");
    }

    #[test]
    fn partial_flags_error() {
        let result = resolve_credentials(Some("http://localhost:3000"), None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn invalid_server_url_error() {
        let result = resolve_credentials(Some("not a url"), Some("token"));
        assert!(result.is_err());
    }

    #[test]
    fn credential_source_display() {
        assert_eq!(format!("{}", CredentialSource::Flags), "CLI flags");
        assert_eq!(
            format!("{}", CredentialSource::Environment),
            "environment variables"
        );
        assert_eq!(format!("{}", CredentialSource::ConfigFile), "config file");
    }
}
