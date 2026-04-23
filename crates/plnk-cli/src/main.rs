use clap::Parser;

mod app;
mod commands;
mod help;
mod input;
mod output;

use app::{App, Command};
use output::render_error;
use plnk_core::api::PlankaClientV1;
use plnk_core::auth::{HttpConfig, read_config, resolve_credentials};
use plnk_core::client::HttpClient;
use plnk_core::error::PlankaError;
use plnk_core::transport::TransportPolicy;

fn init_tracing(verbosity: u8, quiet: bool) {
    use tracing_subscriber::EnvFilter;

    let level = if quiet {
        "off"
    } else {
        match verbosity {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(level))
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TransportOverrides {
    max_in_flight: Option<usize>,
    rate_limit: Option<u32>,
    burst: Option<u32>,
    retry_attempts: Option<u32>,
    retry_base_delay_ms: Option<u64>,
    retry_max_delay_ms: Option<u64>,
    no_retry: bool,
}

fn parse_env<T>(name: &str) -> Result<Option<T>, PlankaError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(raw) => raw
            .parse::<T>()
            .map(Some)
            .map_err(|e| PlankaError::InvalidOptionValue {
                field: name.to_string(),
                message: e.to_string(),
            }),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(PlankaError::InvalidOptionValue {
            field: name.to_string(),
            message: "must be valid UTF-8".to_string(),
        }),
    }
}

fn validate_range<T>(field: &str, value: T, min: T, max: T) -> Result<T, PlankaError>
where
    T: PartialOrd + Copy + std::fmt::Display,
{
    if value < min || value > max {
        return Err(PlankaError::InvalidOptionValue {
            field: field.to_string(),
            message: format!("must be between {min} and {max}"),
        });
    }
    Ok(value)
}

fn apply_overrides(
    policy: &mut TransportPolicy,
    overrides: &TransportOverrides,
    field_prefix: &str,
) -> Result<(), PlankaError> {
    if let Some(value) = overrides.max_in_flight {
        policy.max_in_flight =
            validate_range(&format!("{field_prefix}max_in_flight"), value, 1, 64)?;
    }
    if let Some(value) = overrides.rate_limit {
        policy.rate_limit_per_second = Some(validate_range(
            &format!("{field_prefix}rate_limit"),
            value,
            1,
            1_000,
        )?);
    }
    if let Some(value) = overrides.burst {
        policy.burst_size = Some(validate_range(
            &format!("{field_prefix}burst"),
            value,
            1,
            1_000,
        )?);
    }
    if let Some(value) = overrides.retry_attempts {
        policy.retry_attempts =
            validate_range(&format!("{field_prefix}retry_attempts"), value, 0, 10)?;
    }
    if let Some(value) = overrides.retry_base_delay_ms {
        policy.retry_base_delay_ms = validate_range(
            &format!("{field_prefix}retry_base_delay_ms"),
            value,
            1,
            60_000,
        )?;
    }
    if let Some(value) = overrides.retry_max_delay_ms {
        policy.retry_max_delay_ms = validate_range(
            &format!("{field_prefix}retry_max_delay_ms"),
            value,
            1,
            60_000,
        )?;
    }
    if overrides.no_retry {
        policy.retry_attempts = 0;
    }

    Ok(())
}

fn merge_transport_policy(
    config_http: Option<HttpConfig>,
    env: &TransportOverrides,
    flags: &TransportOverrides,
) -> Result<TransportPolicy, PlankaError> {
    let mut policy = TransportPolicy::default();

    if let Some(http) = config_http {
        apply_overrides(
            &mut policy,
            &TransportOverrides {
                max_in_flight: http.max_in_flight,
                rate_limit: http.rate_limit,
                burst: http.burst,
                retry_attempts: http.retry_attempts,
                retry_base_delay_ms: http.retry_base_delay_ms,
                retry_max_delay_ms: http.retry_max_delay_ms,
                no_retry: false,
            },
            "config.http.",
        )?;
    }

    apply_overrides(&mut policy, env, "")?;
    apply_overrides(&mut policy, flags, "")?;

    policy.validate()?;
    Ok(policy)
}

fn resolve_transport_policy(flags: &TransportOverrides) -> Result<TransportPolicy, PlankaError> {
    let config_http = read_config()?.and_then(|config| config.http);
    let env = TransportOverrides {
        max_in_flight: parse_env::<usize>("PLNK_HTTP_MAX_IN_FLIGHT")?,
        rate_limit: parse_env::<u32>("PLNK_HTTP_RATE_LIMIT")?,
        burst: parse_env::<u32>("PLNK_HTTP_BURST")?,
        retry_attempts: parse_env::<u32>("PLNK_RETRY_ATTEMPTS")?,
        retry_base_delay_ms: parse_env::<u64>("PLNK_RETRY_BASE_DELAY_MS")?,
        retry_max_delay_ms: parse_env::<u64>("PLNK_RETRY_MAX_DELAY_MS")?,
        no_retry: false,
    };

    merge_transport_policy(config_http, &env, flags)
}

/// Build a `PlankaClientV1` from resolved credentials.
fn build_client(
    flag_server: Option<&str>,
    flag_token: Option<&str>,
    transport_policy: &TransportPolicy,
) -> Result<PlankaClientV1, PlankaError> {
    let creds = resolve_credentials(flag_server, flag_token)?;
    let http = HttpClient::with_policy(creds.server, &creds.token, transport_policy.clone())?;
    Ok(PlankaClientV1::new(http))
}

#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() {
    // Machine-readable help: if --help + --output json, render JSON help and exit.
    if help::try_machine_help() {
        return;
    }

    let app = App::parse();

    init_tracing(app.verbose, app.quiet);

    let transport_flags = TransportOverrides {
        max_in_flight: app.http_max_in_flight,
        rate_limit: app.http_rate_limit,
        burst: app.http_burst,
        retry_attempts: app.retry_attempts,
        retry_base_delay_ms: app.retry_base_delay_ms,
        retry_max_delay_ms: app.retry_max_delay_ms,
        no_retry: app.no_retry,
    };

    let Some(command) = app.command else {
        // No subcommand — clap already printed help via derive
        return;
    };

    let transport_policy = match resolve_transport_policy(&transport_flags) {
        Ok(policy) => policy,
        Err(e) => {
            render_error(&e, app.output);
            std::process::exit(e.exit_code());
        }
    };

    let result = match command {
        Command::Init(_cmd) => commands::init::execute(
            app.server.as_deref(),
            app.token.as_deref(),
            app.output,
            app.yes,
        ),
        Command::Auth(auth) => {
            commands::auth::execute(
                auth,
                app.server.as_deref(),
                app.token.as_deref(),
                app.output,
                &transport_policy,
            )
            .await
        }
        // All resource commands need a client
        Command::User(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => commands::user::execute(&client, cmd.action, app.output, app.full).await,
            Err(e) => Err(e),
        },
        Command::Project(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::project::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::Board(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::board::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::List(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::list::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::Card(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => match cmd.action {
                crate::app::CardAction::Label(sub) => {
                    commands::card_label::execute(&client, sub.action, app.output, app.full).await
                }
                crate::app::CardAction::Assignee(sub) => {
                    commands::card_assignee::execute(&client, sub.action, app.output, app.full)
                        .await
                }
                action => {
                    commands::card::execute(&client, action, app.output, app.yes, app.full).await
                }
            },
            Err(e) => Err(e),
        },
        Command::Task(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::task::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::Comment(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::comment::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::Label(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::label::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::Attachment(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::attachment::execute(&client, cmd.action, app.output, app.yes, app.full)
                    .await
            }
            Err(e) => Err(e),
        },
        Command::Membership(cmd) => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::membership::execute(&client, cmd.action, app.output, app.full).await
            }
            Err(e) => Err(e),
        },

        // ── Plural aliases → canonical list actions ─────────────────
        Command::Boards { project } => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::board::execute(
                    &client,
                    app::BoardAction::List { project },
                    app.output,
                    app.yes,
                    app.full,
                )
                .await
            }
            Err(e) => Err(e),
        },
        Command::Lists { board } => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::list::execute(
                    &client,
                    app::ListAction::List { board },
                    app.output,
                    app.yes,
                    app.full,
                )
                .await
            }
            Err(e) => Err(e),
        },
        Command::Cards { list, board, label } => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::card::execute(
                    &client,
                    app::CardAction::List { list, board, label },
                    app.output,
                    app.yes,
                    app.full,
                )
                .await
            }
            Err(e) => Err(e),
        },
        Command::Tasks { card } => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::task::execute(
                    &client,
                    app::TaskAction::List { card },
                    app.output,
                    app.yes,
                    app.full,
                )
                .await
            }
            Err(e) => Err(e),
        },
        Command::Comments { card } => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::comment::execute(
                    &client,
                    app::CommentAction::List { card },
                    app.output,
                    app.yes,
                    app.full,
                )
                .await
            }
            Err(e) => Err(e),
        },
        Command::Labels { board } => match build_client(
            app.server.as_deref(),
            app.token.as_deref(),
            &transport_policy,
        ) {
            Ok(client) => {
                commands::label::execute(
                    &client,
                    app::LabelAction::List { board },
                    app.output,
                    app.yes,
                    app.full,
                )
                .await
            }
            Err(e) => Err(e),
        },
    };

    match result {
        Ok(()) => {}
        Err(e) => {
            render_error(&e, app.output);
            std::process::exit(e.exit_code());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_policy_precedence_is_flags_over_env_over_config_over_defaults() {
        let policy = merge_transport_policy(
            Some(HttpConfig {
                max_in_flight: Some(3),
                rate_limit: Some(4),
                burst: Some(5),
                retry_attempts: Some(1),
                retry_base_delay_ms: Some(200),
                retry_max_delay_ms: Some(400),
            }),
            &TransportOverrides {
                max_in_flight: Some(6),
                rate_limit: Some(7),
                burst: Some(8),
                retry_attempts: Some(2),
                retry_base_delay_ms: Some(300),
                retry_max_delay_ms: Some(500),
                no_retry: false,
            },
            &TransportOverrides {
                max_in_flight: Some(9),
                rate_limit: Some(10),
                burst: Some(11),
                retry_attempts: Some(3),
                retry_base_delay_ms: Some(350),
                retry_max_delay_ms: Some(600),
                no_retry: false,
            },
        )
        .unwrap();

        assert_eq!(policy.max_in_flight, 9);
        assert_eq!(policy.rate_limit_per_second, Some(10));
        assert_eq!(policy.burst_size, Some(11));
        assert_eq!(policy.retry_attempts, 3);
        assert_eq!(policy.retry_base_delay_ms, 350);
        assert_eq!(policy.retry_max_delay_ms, 600);
    }

    #[test]
    fn no_retry_flag_forces_retry_attempts_to_zero() {
        let policy = merge_transport_policy(
            Some(HttpConfig {
                max_in_flight: None,
                rate_limit: None,
                burst: None,
                retry_attempts: Some(4),
                retry_base_delay_ms: None,
                retry_max_delay_ms: None,
            }),
            &TransportOverrides {
                retry_attempts: Some(2),
                ..TransportOverrides::default()
            },
            &TransportOverrides {
                no_retry: true,
                ..TransportOverrides::default()
            },
        )
        .unwrap();

        assert_eq!(policy.retry_attempts, 0);
    }

    #[test]
    fn invalid_transport_flag_range_fails_validation() {
        let err = merge_transport_policy(
            None,
            &TransportOverrides::default(),
            &TransportOverrides {
                max_in_flight: Some(0),
                ..TransportOverrides::default()
            },
        )
        .unwrap_err();

        assert_eq!(err.error_type(), "InvalidOptionValue");
        assert!(err.to_string().contains("max_in_flight"));
    }
}
