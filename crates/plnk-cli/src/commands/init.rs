use std::io::{BufRead, IsTerminal, Write};

use plnk_core::auth::{AuthScheme, ConfigFile, HttpConfig, config_path, read_config, write_config};
use plnk_core::error::PlankaError;
use url::Url;

use crate::app::OutputFormat;

/// Entry point for `plnk init`.
///
/// Interactive-only. Errors if stdin is not a TTY, pointing the caller
/// to flags / env vars for scripted use. Existing config values are
/// shown as defaults so re-running the command is safe; the prompts
/// themselves act as the overwrite confirmation.
pub fn execute(
    flag_server: Option<&str>,
    flag_token: Option<&str>,
    _format: OutputFormat,
    assume_yes: bool,
) -> Result<(), PlankaError> {
    if !std::io::stdin().is_terminal() {
        return Err(PlankaError::InvalidOptionValue {
            field: "plnk init".to_string(),
            message: "`plnk init` is interactive. For non-interactive setup, pass --server and \
                      --token or set PLANKA_SERVER and PLANKA_TOKEN."
                .to_string(),
        });
    }

    let existing = read_config()?;
    let target_path = config_path();

    if existing.is_some() && !assume_yes {
        eprintln!("Found existing config at {}", target_path.display());
        if !prompt_yes_no("Reconfigure?", false)? {
            eprintln!("Left config unchanged.");
            return Ok(());
        }
    } else if existing.is_none() {
        eprintln!(
            "No config file found. Creating a new one at {}",
            target_path.display()
        );
    }

    let server = prompt_server(flag_server, existing.as_ref().map(|c| c.server.as_str()))?;
    let token = prompt_token(flag_token, existing.as_ref().map(|c| c.token.as_str()))?;

    let http = if prompt_yes_no("Configure advanced HTTP settings?", false)? {
        Some(prompt_http(
            existing.as_ref().and_then(|c| c.http.as_ref()),
        )?)
    } else {
        existing.as_ref().and_then(|c| c.http.clone())
    };

    let auth_scheme = auth_scheme_for_token(existing.as_ref(), &token);
    let new_config = ConfigFile {
        server,
        token,
        auth_scheme,
        http,
    };

    write_config(&new_config)?;

    eprintln!();
    eprintln!("Wrote config to {}", target_path.display());
    eprintln!(
        "Edit this file directly, re-run `plnk init`, or set PLANKA_SERVER / PLANKA_TOKEN to override."
    );
    eprintln!("Run `plnk auth status` to verify.");

    Ok(())
}

/// Pick the auth scheme to save alongside `token`.
///
/// `init` only ever prompts for an API key, so a newly entered token is always
/// `ApiKey`. The existing scheme is kept only when the existing token is kept
/// (e.g. pressing Enter at the prompt after an earlier `plnk auth login`).
fn auth_scheme_for_token(existing: Option<&ConfigFile>, token: &str) -> AuthScheme {
    match existing {
        Some(config) if config.token == token => config.auth_scheme,
        _ => AuthScheme::ApiKey,
    }
}

fn prompt_server(flag: Option<&str>, existing: Option<&str>) -> Result<String, PlankaError> {
    if let Some(s) = flag {
        validate_url(s, "--server")?;
        return Ok(s.to_string());
    }

    loop {
        let raw = prompt_line("Server URL", existing)?;
        if raw.is_empty() {
            eprintln!("Server URL is required.");
            continue;
        }
        match validate_url(&raw, "server URL") {
            Ok(()) => return Ok(raw),
            Err(e) => eprintln!("{e}"),
        }
    }
}

fn validate_url(raw: &str, field: &str) -> Result<(), PlankaError> {
    Url::parse(raw).map_err(|e| PlankaError::InvalidOptionValue {
        field: field.to_string(),
        message: format!("Invalid URL: {e}"),
    })?;
    Ok(())
}

fn prompt_token(flag: Option<&str>, existing: Option<&str>) -> Result<String, PlankaError> {
    if let Some(t) = flag {
        return Ok(t.to_string());
    }

    let hint = if existing.is_some() {
        "API token (leave blank to keep current)"
    } else {
        "API token"
    };

    loop {
        let entered = rpassword::prompt_password(format!("{hint}: ")).map_err(|e| {
            PlankaError::FileReadError {
                path: "<stdin>".to_string(),
                source: e,
            }
        })?;

        if entered.is_empty() {
            if let Some(existing) = existing {
                return Ok(existing.to_string());
            }
            eprintln!("Token is required.");
            continue;
        }

        return Ok(entered);
    }
}

fn prompt_http(existing: Option<&HttpConfig>) -> Result<HttpConfig, PlankaError> {
    // Fall back to the built-in transport defaults when no value is
    // configured yet, so the user sees what will take effect if they
    // just hit enter.
    let defaults = plnk_core::transport::TransportPolicy::default();

    Ok(HttpConfig {
        max_in_flight: Some(prompt_number(
            "max in-flight requests",
            existing.and_then(|h| h.max_in_flight),
            defaults.max_in_flight,
        )?),
        rate_limit: Some(prompt_number(
            "rate limit (requests/sec)",
            existing.and_then(|h| h.rate_limit),
            defaults.rate_limit_per_second.unwrap_or(10),
        )?),
        burst: Some(prompt_number(
            "burst size",
            existing.and_then(|h| h.burst),
            defaults.burst_size.unwrap_or(10),
        )?),
        retry_attempts: Some(prompt_number(
            "retry attempts",
            existing.and_then(|h| h.retry_attempts),
            defaults.retry_attempts,
        )?),
        retry_base_delay_ms: Some(prompt_number(
            "retry base delay (ms)",
            existing.and_then(|h| h.retry_base_delay_ms),
            defaults.retry_base_delay_ms,
        )?),
        retry_max_delay_ms: Some(prompt_number(
            "retry max delay (ms)",
            existing.and_then(|h| h.retry_max_delay_ms),
            defaults.retry_max_delay_ms,
        )?),
    })
}

/// Prompt for a numeric value with the existing configured value (if any)
/// or the built-in default shown as the bracketed hint. A blank response
/// accepts whichever value is shown.
fn prompt_number<T>(label: &str, existing: Option<T>, fallback: T) -> Result<T, PlankaError>
where
    T: std::str::FromStr + std::fmt::Display + Copy,
    T::Err: std::fmt::Display,
{
    let shown = existing.unwrap_or(fallback);
    loop {
        let entered = prompt_line(label, Some(&shown.to_string()))?;
        if entered.is_empty() || entered == shown.to_string() {
            return Ok(shown);
        }
        match entered.parse::<T>() {
            Ok(v) => return Ok(v),
            Err(e) => eprintln!("Not a valid number: {e}"),
        }
    }
}

fn prompt_line(label: &str, default: Option<&str>) -> Result<String, PlankaError> {
    let suffix = match default {
        Some(d) if !d.is_empty() => format!(" [{d}]"),
        _ => String::new(),
    };
    eprint!("{label}{suffix}: ");
    std::io::stderr()
        .flush()
        .map_err(|e| PlankaError::FileReadError {
            path: "<stderr>".to_string(),
            source: e,
        })?;

    let mut buf = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut buf)
        .map_err(|e| PlankaError::FileReadError {
            path: "<stdin>".to_string(),
            source: e,
        })?;

    let trimmed = buf.trim().to_string();
    if trimmed.is_empty() {
        if let Some(d) = default {
            if !d.is_empty() {
                return Ok(d.to_string());
            }
        }
    }
    Ok(trimmed)
}

fn prompt_yes_no(question: &str, default_yes: bool) -> Result<bool, PlankaError> {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    eprint!("{question} {hint} ");
    std::io::stderr()
        .flush()
        .map_err(|e| PlankaError::FileReadError {
            path: "<stderr>".to_string(),
            source: e,
        })?;

    let mut buf = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut buf)
        .map_err(|e| PlankaError::FileReadError {
            path: "<stdin>".to_string(),
            source: e,
        })?;

    Ok(parse_yes_no(&buf, default_yes))
}

fn parse_yes_no(input: &str, default_yes: bool) -> bool {
    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default_yes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yes_no_explicit_yes() {
        assert!(parse_yes_no("y\n", false));
        assert!(parse_yes_no("YES", false));
        assert!(parse_yes_no(" Y ", false));
    }

    #[test]
    fn parse_yes_no_explicit_no() {
        assert!(!parse_yes_no("n", true));
        assert!(!parse_yes_no("NO", true));
    }

    #[test]
    fn parse_yes_no_empty_uses_default() {
        assert!(parse_yes_no("", true));
        assert!(!parse_yes_no("", false));
        assert!(parse_yes_no("\n", true));
    }

    #[test]
    fn parse_yes_no_unknown_uses_default() {
        assert!(parse_yes_no("maybe", true));
        assert!(!parse_yes_no("maybe", false));
    }

    fn config_with(token: &str, auth_scheme: AuthScheme) -> ConfigFile {
        ConfigFile {
            server: "http://localhost:3000".to_string(),
            token: token.to_string(),
            auth_scheme,
            http: None,
        }
    }

    #[test]
    fn auth_scheme_kept_when_existing_token_is_kept() {
        let existing = config_with("login-token", AuthScheme::Bearer);
        assert_eq!(
            auth_scheme_for_token(Some(&existing), "login-token"),
            AuthScheme::Bearer
        );
    }

    #[test]
    fn auth_scheme_resets_to_api_key_for_a_new_token() {
        let existing = config_with("login-token", AuthScheme::Bearer);
        assert_eq!(
            auth_scheme_for_token(Some(&existing), "fresh-api-key"),
            AuthScheme::ApiKey
        );
    }

    #[test]
    fn auth_scheme_defaults_to_api_key_without_existing_config() {
        assert_eq!(
            auth_scheme_for_token(None, "fresh-api-key"),
            AuthScheme::ApiKey
        );
    }

    #[test]
    fn validate_url_accepts_http_and_https() {
        assert!(validate_url("http://localhost:3000", "test").is_ok());
        assert!(validate_url("https://planka.example.com", "test").is_ok());
    }

    #[test]
    fn validate_url_rejects_garbage() {
        assert!(validate_url("not a url", "test").is_err());
        assert!(validate_url("", "test").is_err());
    }
}
