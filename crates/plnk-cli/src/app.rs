use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "plnk",
    version,
    about = "CLI for Planka kanban boards",
    long_about = "Deterministic, scriptable, hierarchy-aware CLI for Planka project management.\n\n\
                  Grammar: plnk <resource> <action> [target] [flags]"
)]
pub struct App {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Planka server URL
    #[arg(long, global = true, env = "PLANKA_SERVER")]
    pub server: Option<String>,

    /// API token
    #[arg(long, global = true, env = "PLANKA_TOKEN")]
    pub token: Option<String>,

    /// Output format
    #[arg(long, global = true, default_value = "table", value_enum)]
    pub output: OutputFormat,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress all output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Skip confirmation prompts
    #[arg(long, global = true)]
    pub yes: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Markdown,
}

/// Top-level command groups. Each resource gets its own subcommand.
/// Resource commands are added in subsequent milestone tasks.
#[derive(Subcommand)]
pub enum Command {
    /// Manage authentication
    Auth(AuthCommand),
}

/// Auth subcommands.
#[derive(Parser)]
pub struct AuthCommand {
    #[command(subcommand)]
    pub action: AuthAction,
}

#[derive(Subcommand)]
pub enum AuthAction {
    /// Log in with email and password
    Login {
        /// Planka server URL (overrides --server)
        #[arg(long)]
        server: Option<String>,
        /// Email address
        #[arg(long)]
        email: Option<String>,
        /// Password (will prompt if not given)
        #[arg(long)]
        password: Option<String>,
    },
    /// Set an API token directly
    Token(TokenCommand),
    /// Show current authenticated user
    Whoami,
    /// Remove stored credentials
    Logout,
    /// Show credential source and validation status
    Status,
}

#[derive(Parser)]
pub struct TokenCommand {
    #[command(subcommand)]
    pub action: TokenAction,
}

#[derive(Subcommand)]
pub enum TokenAction {
    /// Store an API token in the config file
    Set {
        /// The API token
        token: String,
        /// Planka server URL
        #[arg(long)]
        server: Option<String>,
    },
}
