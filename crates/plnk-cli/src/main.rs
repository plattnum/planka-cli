use clap::Parser;

mod app;
mod commands;
mod input;
mod output;

use app::{App, Command};
use output::render_error;
use plnk_core::api::PlankaClientV1;
use plnk_core::auth::resolve_credentials;
use plnk_core::client::HttpClient;

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

/// Build a `PlankaClientV1` from resolved credentials.
fn build_client(
    flag_server: Option<&str>,
    flag_token: Option<&str>,
) -> Result<PlankaClientV1, plnk_core::error::PlankaError> {
    let creds = resolve_credentials(flag_server, flag_token)?;
    let http = HttpClient::new(creds.server, &creds.token)?;
    Ok(PlankaClientV1::new(http))
}

#[tokio::main]
async fn main() {
    let app = App::parse();

    init_tracing(app.verbose, app.quiet);

    let Some(command) = app.command else {
        // No subcommand — clap already printed help via derive
        return;
    };

    let result = match command {
        Command::Auth(auth) => {
            commands::auth::execute(
                auth,
                app.server.as_deref(),
                app.token.as_deref(),
                app.output,
            )
            .await
        }
        // All resource commands need a client
        Command::User(cmd) => match build_client(app.server.as_deref(), app.token.as_deref()) {
            Ok(client) => commands::user::execute(&client, cmd.action, app.output, app.full).await,
            Err(e) => Err(e),
        },
        Command::Project(cmd) => match build_client(app.server.as_deref(), app.token.as_deref()) {
            Ok(client) => {
                commands::project::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::Board(cmd) => match build_client(app.server.as_deref(), app.token.as_deref()) {
            Ok(client) => {
                commands::board::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::List(cmd) => match build_client(app.server.as_deref(), app.token.as_deref()) {
            Ok(client) => {
                commands::list::execute(&client, cmd.action, app.output, app.yes, app.full).await
            }
            Err(e) => Err(e),
        },
        Command::Card(cmd) => match build_client(app.server.as_deref(), app.token.as_deref()) {
            Ok(client) => {
                commands::card::execute(&client, cmd.action, app.output, app.yes, app.full).await
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
