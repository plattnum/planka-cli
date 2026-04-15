use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::PlankaError;

/// Config file contents (`~/.config/planka/config.toml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub server: String,
    pub token: String,
}

/// Resolve the config file path using the spec's precedence:
/// 1. `$PLANKA_CONFIG` (explicit override)
/// 2. `$XDG_CONFIG_HOME/planka/config.toml`
/// 3. `~/.config/planka/config.toml`
pub fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var("PLANKA_CONFIG") {
        return PathBuf::from(path);
    }

    let config_dir = dirs::config_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
    });

    config_dir.join("planka").join("config.toml")
}

/// Read the config file. Returns `Ok(None)` if the file doesn't exist.
///
/// # Errors
/// Returns `PlankaError` if the file exists but can't be read or parsed.
pub fn read_config() -> Result<Option<ConfigFile>, PlankaError> {
    let path = config_path();

    if !path.exists() {
        return Ok(None);
    }

    check_permissions(&path);

    let content = std::fs::read_to_string(&path).map_err(|e| PlankaError::FileReadError {
        path: path.display().to_string(),
        source: e,
    })?;

    let config: ConfigFile = toml::from_str(&content)?;
    Ok(Some(config))
}

/// Write credentials to the config file with `0600` permissions.
///
/// # Errors
/// Returns `PlankaError` if the file can't be written.
pub fn write_config(config: &ConfigFile) -> Result<(), PlankaError> {
    let path = config_path();

    // Create parent directories
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;

    // Set file permissions to 0600 on unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

/// Delete the config file (for logout).
///
/// # Errors
/// Returns `PlankaError` if the file exists but can't be deleted.
pub fn delete_config() -> Result<(), PlankaError> {
    let path = config_path();

    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    Ok(())
}

/// Warn on stderr if the config file has permissions broader than 0600.
fn check_permissions(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o600 {
                warn!(
                    "Config file {} has permissions {:04o} (expected 0600). \
                     Run: chmod 600 {}",
                    path.display(),
                    mode,
                    path.display()
                );
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let config = ConfigFile {
            server: "http://localhost:3000".to_string(),
            token: "test-token-123".to_string(),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: ConfigFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.server, config.server);
        assert_eq!(parsed.token, config.token);
    }

    #[test]
    fn config_path_default() {
        // Just verify it returns a path ending in config.toml
        let path = config_path();
        assert!(path.ends_with("planka/config.toml"));
    }
}
