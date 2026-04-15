use clap::Parser;

mod app;
mod commands;
mod input;
mod output;

use app::{App, Command};
use output::render_error;

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
    };

    match result {
        Ok(()) => {}
        Err(e) => {
            render_error(&e, app.output);
            std::process::exit(e.exit_code());
        }
    }
}
