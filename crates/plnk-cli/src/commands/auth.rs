use plnk_core::auth::{
    self, ConfigFile, delete_config, read_config, resolve_credentials, validate_token_with_policy,
    write_config,
};
use plnk_core::error::PlankaError;
use plnk_core::transport::TransportPolicy;

use crate::app::{AuthAction, AuthCommand, OutputFormat, TokenAction};
use crate::output::{render_item, render_message};

pub async fn execute(
    cmd: AuthCommand,
    flag_server: Option<&str>,
    flag_token: Option<&str>,
    format: OutputFormat,
    transport_policy: &TransportPolicy,
) -> Result<(), PlankaError> {
    match cmd.action {
        AuthAction::Login {
            server,
            email,
            password,
        } => {
            do_login(
                server.as_deref().or(flag_server),
                email.as_deref(),
                password.as_deref(),
                format,
                transport_policy,
            )
            .await
        }
        AuthAction::Token(token_cmd) => match token_cmd.action {
            TokenAction::Set { token, server } => {
                do_token_set(&token, server.as_deref().or(flag_server), format)
            }
        },
        AuthAction::Whoami => do_whoami(flag_server, flag_token, format, transport_policy).await,
        AuthAction::Logout => do_logout(format),
        AuthAction::Status => do_status(flag_server, flag_token, format, transport_policy).await,
    }
}

async fn do_login(
    server: Option<&str>,
    email: Option<&str>,
    password: Option<&str>,
    format: OutputFormat,
    transport_policy: &TransportPolicy,
) -> Result<(), PlankaError> {
    // Resolve server URL
    let server_url = if let Some(s) = server {
        s.to_string()
    } else if let Ok(s) = std::env::var("PLANKA_SERVER") {
        s
    } else {
        eprint!("Server URL: ");
        let mut buf = String::new();
        std::io::stdin()
            .read_line(&mut buf)
            .map_err(|e| PlankaError::FileReadError {
                path: "<stdin>".to_string(),
                source: e,
            })?;
        buf.trim().to_string()
    };

    let server = url::Url::parse(&server_url).map_err(|e| PlankaError::InvalidOptionValue {
        field: "--server".to_string(),
        message: format!("Invalid URL: {e}"),
    })?;

    // Resolve email
    let email_str = if let Some(e) = email {
        e.to_string()
    } else {
        eprint!("Email: ");
        let mut buf = String::new();
        std::io::stdin()
            .read_line(&mut buf)
            .map_err(|e| PlankaError::FileReadError {
                path: "<stdin>".to_string(),
                source: e,
            })?;
        buf.trim().to_string()
    };

    // Resolve password
    let password_str = if let Some(p) = password {
        p.to_string()
    } else {
        rpassword::prompt_password("Password: ").map_err(|e| PlankaError::FileReadError {
            path: "<stderr>".to_string(),
            source: e,
        })?
    };

    // Exchange credentials for token
    let token =
        auth::login_with_policy(&server, &email_str, &password_str, transport_policy.clone())
            .await?;

    // Save to config, preserving any existing transport settings.
    let existing_http = read_config()?.and_then(|config| config.http);
    write_config(&ConfigFile {
        server: server_url,
        token: token.clone(),
        http: existing_http,
    })?;

    // Validate and show user identity
    let user = validate_token_with_policy(&server, &token, transport_policy.clone()).await?;

    if format == OutputFormat::Json {
        render_item(&user, format, false)?;
    } else {
        eprintln!(
            "Logged in as {} ({})",
            user.name,
            user.email.as_deref().unwrap_or("no email")
        );
    }

    Ok(())
}

fn do_token_set(
    token: &str,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<(), PlankaError> {
    let server_url = if let Some(s) = server {
        s.to_string()
    } else if let Ok(s) = std::env::var("PLANKA_SERVER") {
        s
    } else if let Some(config) = read_config()? {
        config.server
    } else {
        return Err(PlankaError::AuthenticationFailed {
            message: "No server URL configured. Pass --server or set PLANKA_SERVER.".to_string(),
        });
    };

    let existing_http = read_config()?.and_then(|config| config.http);
    write_config(&ConfigFile {
        server: server_url,
        token: token.to_string(),
        http: existing_http,
    })?;

    render_message("Token saved.", format)?;
    Ok(())
}

async fn do_whoami(
    flag_server: Option<&str>,
    flag_token: Option<&str>,
    format: OutputFormat,
    transport_policy: &TransportPolicy,
) -> Result<(), PlankaError> {
    let creds = resolve_credentials(flag_server, flag_token)?;
    let user =
        validate_token_with_policy(&creds.server, &creds.token, transport_policy.clone()).await?;
    render_item(&user, format, false)?;
    Ok(())
}

fn do_logout(format: OutputFormat) -> Result<(), PlankaError> {
    delete_config()?;
    render_message("Logged out. Stored credentials removed.", format)?;
    Ok(())
}

async fn do_status(
    flag_server: Option<&str>,
    flag_token: Option<&str>,
    format: OutputFormat,
    transport_policy: &TransportPolicy,
) -> Result<(), PlankaError> {
    let creds = match resolve_credentials(flag_server, flag_token) {
        Ok(c) => c,
        Err(e) => {
            if format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "success": true,
                        "data": {
                            "authenticated": false,
                            "source": null,
                            "message": e.to_string()
                        }
                    })
                );
            } else {
                println!("Not authenticated: {e}");
            }
            return Ok(());
        }
    };

    // Try validating the token
    let valid =
        validate_token_with_policy(&creds.server, &creds.token, transport_policy.clone()).await;

    if format == OutputFormat::Json {
        let (authenticated, user_name) = match &valid {
            Ok(user) => (true, Some(user.name.clone())),
            Err(_) => (false, None),
        };
        println!(
            "{}",
            serde_json::json!({
                "success": true,
                "data": {
                    "authenticated": authenticated,
                    "source": creds.source.to_string(),
                    "server": creds.server.as_str(),
                    "user": user_name,
                }
            })
        );
    } else {
        println!("Server: {}", creds.server);
        println!("Source: {}", creds.source);
        match valid {
            Ok(user) => println!("User: {} ({})", user.name, user.role),
            Err(e) => println!("Token invalid: {e}"),
        }
    }

    Ok(())
}
