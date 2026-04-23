use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::PlankaError;

/// Optional transport tuning in the config file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HttpConfig {
    pub max_in_flight: Option<usize>,
    pub rate_limit: Option<u32>,
    pub burst: Option<u32>,
    pub retry_attempts: Option<u32>,
    pub retry_base_delay_ms: Option<u64>,
    pub retry_max_delay_ms: Option<u64>,
}

/// Config file contents.
///
/// Default location on every OS: `~/.config/plnk/config.toml`, honoring
/// `XDG_CONFIG_HOME` when set. `PLANKA_CONFIG` overrides the location
/// entirely. `plnk` is a CLI tool, so it uses XDG on macOS and Windows too —
/// not the GUI-app directories those platforms default to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub server: String,
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpConfig>,
}

/// Resolve the config file path.
///
/// Precedence:
/// 1. `$PLANKA_CONFIG` (explicit override)
/// 2. `$XDG_CONFIG_HOME/plnk/config.toml`
/// 3. `~/.config/plnk/config.toml`
///
/// XDG on every OS — we're a CLI, not a GUI app, so we do not follow
/// platform-specific config dirs (`~/Library/Application Support`,
/// `%APPDATA%`) the way `dirs::config_dir()` does.
pub fn config_path() -> PathBuf {
    resolve_config_path(
        std::env::var("PLANKA_CONFIG").ok().as_deref(),
        std::env::var("XDG_CONFIG_HOME").ok().as_deref(),
        dirs::home_dir().as_deref(),
    )
}

/// Pure resolver behind [`config_path`], factored out so callers can pass
/// explicit overrides (primarily for unit tests without env mutation).
fn resolve_config_path(
    planka_config: Option<&str>,
    xdg_config_home: Option<&str>,
    home: Option<&Path>,
) -> PathBuf {
    if let Some(path) = planka_config.filter(|s| !s.is_empty()) {
        return PathBuf::from(path);
    }

    let base = xdg_config_home.filter(|s| !s.is_empty()).map_or_else(
        || {
            home.map_or_else(|| PathBuf::from("."), Path::to_path_buf)
                .join(".config")
        },
        PathBuf::from,
    );

    base.join("plnk").join("config.toml")
}

/// Read the config file. Returns `Ok(None)` if the file doesn't exist.
///
/// On first call after the directory rename from `planka/` to `plnk/`,
/// this transparently migrates an existing legacy config file (see
/// [`migrate_legacy_config`]). Migration is skipped entirely when
/// `PLANKA_CONFIG` is set.
///
/// # Errors
/// Returns `PlankaError` if the file exists but can't be read or parsed.
pub fn read_config() -> Result<Option<ConfigFile>, PlankaError> {
    let path = config_path();

    if !path.exists() && std::env::var_os("PLANKA_CONFIG").is_none() {
        migrate_legacy_config(&path);
    }

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

    set_owner_only_permissions(&path)?;

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
fn check_permissions(path: &Path) {
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

fn set_owner_only_permissions(path: &Path) -> Result<(), PlankaError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Legacy config paths from before the `planka/` → `plnk/` rename.
///
/// Returns the paths to check in order. `dirs::config_dir()/planka/config.toml`
/// captures the old per-OS behavior (macOS: `~/Library/Application Support`,
/// Windows: `%APPDATA%`, Linux: `~/.config`). We also explicitly check
/// `~/.config/planka/config.toml` in case a user pointed there manually
/// on a platform where `dirs::config_dir()` returns something else.
fn legacy_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(dir) = dirs::config_dir() {
        paths.push(dir.join("planka").join("config.toml"));
    }

    if let Some(home) = dirs::home_dir() {
        let xdg_legacy = home.join(".config").join("planka").join("config.toml");
        if !paths.contains(&xdg_legacy) {
            paths.push(xdg_legacy);
        }
    }

    paths
}

/// One-time migration from the old `planka/` directory to the new `plnk/` path.
///
/// If any legacy config file exists and the new path is absent, copy the
/// first legacy file found to the new location with `0600` permissions and
/// print a single-line notice on stderr. The legacy file is left in place
/// for the user to remove after verifying.
///
/// Best-effort: any IO error during migration is reported on stderr and
/// swallowed — a missing migration should never block `read_config` from
/// trying a fresh setup flow.
fn migrate_legacy_config(new_path: &Path) {
    for legacy in legacy_config_paths() {
        if legacy == new_path || !legacy.exists() {
            continue;
        }

        if let Some(parent) = new_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "warning: could not create {} for config migration: {e}",
                    parent.display()
                );
                return;
            }
        }

        if let Err(e) = std::fs::copy(&legacy, new_path) {
            eprintln!(
                "warning: could not migrate config from {} to {}: {e}",
                legacy.display(),
                new_path.display()
            );
            return;
        }

        if let Err(e) = set_owner_only_permissions(new_path) {
            eprintln!(
                "warning: migrated config to {} but could not set 0600 permissions: {e}",
                new_path.display()
            );
        }

        eprintln!(
            "migrated config from {} to {}",
            legacy.display(),
            new_path.display()
        );
        return;
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
            http: Some(HttpConfig {
                max_in_flight: Some(8),
                rate_limit: Some(10),
                burst: Some(10),
                retry_attempts: Some(2),
                retry_base_delay_ms: Some(250),
                retry_max_delay_ms: Some(2_000),
            }),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: ConfigFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.server, config.server);
        assert_eq!(parsed.token, config.token);
        assert_eq!(parsed.http, config.http);
    }

    #[test]
    fn resolve_uses_planka_config_when_set() {
        let path = resolve_config_path(
            Some("/opt/custom/plnk.toml"),
            Some("/xdg"),
            Some(Path::new("/home/u")),
        );
        assert_eq!(path, PathBuf::from("/opt/custom/plnk.toml"));
    }

    #[test]
    fn resolve_ignores_empty_planka_config() {
        let path = resolve_config_path(Some(""), None, Some(Path::new("/home/u")));
        assert_eq!(path, PathBuf::from("/home/u/.config/plnk/config.toml"));
    }

    #[test]
    fn resolve_uses_xdg_config_home_when_set() {
        let path = resolve_config_path(None, Some("/xdg"), Some(Path::new("/home/u")));
        assert_eq!(path, PathBuf::from("/xdg/plnk/config.toml"));
    }

    #[test]
    fn resolve_ignores_empty_xdg_config_home() {
        let path = resolve_config_path(None, Some(""), Some(Path::new("/home/u")));
        assert_eq!(path, PathBuf::from("/home/u/.config/plnk/config.toml"));
    }

    #[test]
    fn resolve_defaults_to_home_dot_config() {
        let path = resolve_config_path(None, None, Some(Path::new("/home/alice")));
        assert_eq!(path, PathBuf::from("/home/alice/.config/plnk/config.toml"));
    }

    #[test]
    fn resolve_without_home_falls_back_to_cwd() {
        let path = resolve_config_path(None, None, None);
        assert_eq!(path, PathBuf::from("./.config/plnk/config.toml"));
    }
}
