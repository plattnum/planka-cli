use clap::{Parser, Subcommand};

#[derive(Parser)]
#[allow(clippy::struct_excessive_bools)]
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

    /// Show all fields (default output is trimmed to essentials)
    #[arg(long, global = true)]
    pub full: bool,

    /// Max in-flight HTTP requests per process
    #[arg(long = "http-max-in-flight", global = true)]
    pub http_max_in_flight: Option<usize>,

    /// Sustained HTTP request rate limit (requests/sec)
    #[arg(long = "http-rate-limit", global = true)]
    pub http_rate_limit: Option<u32>,

    /// HTTP rate-limit burst size
    #[arg(long = "http-burst", global = true)]
    pub http_burst: Option<u32>,

    /// Retry attempts after the initial HTTP request
    #[arg(long = "retry-attempts", global = true)]
    pub retry_attempts: Option<u32>,

    /// Base retry delay in milliseconds
    #[arg(long = "retry-base-delay-ms", global = true)]
    pub retry_base_delay_ms: Option<u64>,

    /// Maximum retry delay in milliseconds
    #[arg(long = "retry-max-delay-ms", global = true)]
    pub retry_max_delay_ms: Option<u64>,

    /// Disable automatic HTTP retries
    #[arg(long = "no-retry", global = true)]
    pub no_retry: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Markdown,
}

/// Top-level command groups. Each resource gets its own subcommand.
#[derive(Subcommand)]
pub enum Command {
    /// Interactive bootstrap — prompts for server/token and writes the config file
    Init(InitCommand),
    /// Manage authentication
    Auth(AuthCommand),
    /// Manage users
    User(UserCommand),
    /// Manage projects
    Project(ProjectCommand),
    /// Manage boards
    Board(BoardCommand),
    /// Manage lists
    List(ListCommand),
    /// Manage cards
    Card(CardCommand),
    /// Manage tasks (checklist items on cards)
    Task(TaskCommand),
    /// Manage comments on cards
    Comment(CommentCommand),
    /// Manage board labels
    Label(LabelCommand),
    /// Manage attachments on cards
    Attachment(AttachmentCommand),
    /// Manage project/board memberships
    Membership(MembershipCommand),

    // ── Plural aliases (spec section 3.5) ───────────────────────────
    // Hidden from --help. Map to `<resource> list` with the same args.
    /// Alias for `board list --project <id>`
    #[command(hide = true)]
    Boards {
        /// Parent project ID
        #[arg(long)]
        project: String,
    },
    /// Alias for `list list --board <id>`
    #[command(hide = true)]
    Lists {
        /// Parent board ID
        #[arg(long)]
        board: String,
    },
    /// Alias for `card list --list <id>` / `card list --board <id>`
    #[command(hide = true)]
    Cards {
        /// Parent list ID
        #[arg(long, group = "scope")]
        list: Option<String>,
        /// Parent board ID
        #[arg(long, group = "scope")]
        board: Option<String>,
        /// Board-scoped label ID or name (repeat for AND semantics; use an ID to avoid ambiguity)
        #[arg(long = "label")]
        label: Vec<String>,
    },
    /// Alias for `task list --card <id>`
    #[command(hide = true)]
    Tasks {
        /// Parent card ID
        #[arg(long)]
        card: String,
    },
    /// Alias for `comment list --card <id>`
    #[command(hide = true)]
    Comments {
        /// Parent card ID
        #[arg(long)]
        card: String,
    },
    /// Alias for `label list --board <id>`
    #[command(hide = true)]
    Labels {
        /// Parent board ID
        #[arg(long)]
        board: String,
    },
}

// ── Init ─────────────────────────────────────────────────────────────────

/// `plnk init` — interactive first-run config bootstrap.
///
/// Walks the user through selecting a server URL, API token, and
/// optional transport tuning, then writes the result to the config file
/// (see `plnk-core` config path resolution). Re-running is supported;
/// existing values are shown as defaults and can be kept or replaced.
///
/// Scripts and CI should continue to use flags and env vars — this
/// command is interactive-only and errors out when stdin is not a TTY.
///
/// The global `--server`/`--token` flags (and `PLANKA_SERVER` /
/// `PLANKA_TOKEN`) pre-fill the corresponding prompts, so
/// `plnk --server URL --token TKN init` still walks through the
/// remaining questions interactively.
#[derive(Parser)]
pub struct InitCommand {}

// ── Auth ─────────────────────────────────────────────────────────────────

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

// ── User ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct UserCommand {
    #[command(subcommand)]
    pub action: UserAction,
}

#[derive(Subcommand)]
pub enum UserAction {
    /// List all users
    List,
    /// Get a user by ID
    Get {
        /// User ID
        id: String,
    },
}

// ── Project ──────────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct ProjectCommand {
    #[command(subcommand)]
    pub action: ProjectAction,
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// List all projects
    List,
    /// Get a project by ID
    Get {
        /// Project ID
        id: String,
    },
    /// Get the full snapshot (item + included) for a project — JSON only
    Snapshot {
        /// Project ID
        id: String,
    },
    /// Find projects by name (unscoped — projects are the root resource)
    Find {
        /// Project name to search for
        #[arg(long)]
        name: String,
    },
    /// Create a new project
    Create {
        /// Project name
        #[arg(long)]
        name: String,
    },
    /// Update a project
    Update {
        /// Project ID
        id: String,
        /// New project name
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete a project
    Delete {
        /// Project ID
        id: String,
    },
}

// ── Board ────────────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct BoardCommand {
    #[command(subcommand)]
    pub action: BoardAction,
}

#[derive(Subcommand)]
pub enum BoardAction {
    /// List boards in a project
    List {
        /// Parent project ID
        #[arg(long)]
        project: String,
    },
    /// Get a board by ID
    Get {
        /// Board ID
        id: String,
    },
    /// Get the full snapshot (item + included) for a board — JSON only
    Snapshot {
        /// Board ID
        id: String,
    },
    /// Find boards by name within a project
    Find {
        /// Parent project ID
        #[arg(long)]
        project: String,
        /// Board name to search for
        #[arg(long)]
        name: String,
    },
    /// Create a new board
    Create {
        /// Parent project ID
        #[arg(long)]
        project: String,
        /// Board name
        #[arg(long)]
        name: String,
    },
    /// Update a board
    Update {
        /// Board ID
        id: String,
        /// New board name
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete a board
    Delete {
        /// Board ID
        id: String,
    },
}

// ── List ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct ListCommand {
    #[command(subcommand)]
    pub action: ListAction,
}

#[derive(Subcommand)]
pub enum ListAction {
    /// List lists in a board
    List {
        /// Parent board ID
        #[arg(long)]
        board: String,
    },
    /// Get a list by ID
    Get {
        /// List ID
        id: String,
    },
    /// Find lists by name within a board
    Find {
        /// Parent board ID
        #[arg(long)]
        board: String,
        /// List name to search for
        #[arg(long)]
        name: String,
    },
    /// Create a new list
    Create {
        /// Parent board ID
        #[arg(long)]
        board: String,
        /// List name
        #[arg(long)]
        name: String,
    },
    /// Update a list
    Update {
        /// List ID
        id: String,
        /// New list name
        #[arg(long)]
        name: Option<String>,
        /// New position
        #[arg(long)]
        position: Option<f64>,
    },
    /// Move a list to a new position
    Move {
        /// List ID
        id: String,
        /// Target position
        #[arg(long)]
        to_position: f64,
    },
    /// Delete a list
    Delete {
        /// List ID
        id: String,
    },
}

// ── Card ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct CardCommand {
    #[command(subcommand)]
    pub action: CardAction,
}

#[derive(Subcommand)]
pub enum CardAction {
    /// List cards in a list or across a board
    List {
        /// Parent list ID
        #[arg(long, group = "scope")]
        list: Option<String>,
        /// Parent board ID
        #[arg(long, group = "scope")]
        board: Option<String>,
        /// Board-scoped label ID or name (repeat for AND semantics; use an ID to avoid ambiguity)
        #[arg(long = "label")]
        label: Vec<String>,
    },
    /// Get a card by ID
    Get {
        /// Card ID
        id: String,
    },
    /// Get multiple cards by exact ID
    GetMany {
        /// Exact card ID (repeat for multiple cards)
        #[arg(long = "id", required = true)]
        id: Vec<String>,
        /// Max concurrent card fetches
        #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=16))]
        concurrency: u8,
        /// Treat missing card IDs as non-fatal and report them in JSON metadata
        #[arg(long)]
        allow_missing: bool,
    },
    /// Get the full snapshot (item + included) for a card — JSON only
    Snapshot {
        /// Card ID
        id: String,
    },
    /// Find cards by title and/or label within a scope
    Find {
        /// Search within a list
        #[arg(long, group = "scope")]
        list: Option<String>,
        /// Search within a board
        #[arg(long, group = "scope")]
        board: Option<String>,
        /// Search within a project
        #[arg(long, group = "scope")]
        project: Option<String>,
        /// Card title to search for
        #[arg(long)]
        title: Option<String>,
        /// Board-scoped label ID or name (repeat for AND semantics; use an ID to avoid ambiguity)
        #[arg(long = "label")]
        label: Vec<String>,
    },
    /// Create a new card
    Create {
        /// Parent list ID
        #[arg(long)]
        list: String,
        /// Card title
        #[arg(long)]
        title: String,
        /// Card description (literal, "-" for stdin, "@file" for file)
        #[arg(long)]
        description: Option<String>,
        /// Position: "top", "bottom", or numeric
        #[arg(long)]
        position: Option<String>,
    },
    /// Update a card
    Update {
        /// Card ID
        id: String,
        /// New card title
        #[arg(long)]
        title: Option<String>,
        /// New description (literal, "-" for stdin, "@file" for file)
        #[arg(long)]
        description: Option<String>,
    },
    /// Move a card to a different list (optionally on a different board)
    Move {
        /// Card ID
        id: String,
        /// Target list ID
        #[arg(long)]
        to_list: String,
        /// Target board ID — required when moving across boards
        #[arg(long)]
        to_board: Option<String>,
        /// Position: "top", "bottom", or numeric
        #[arg(long)]
        position: Option<String>,
    },
    /// Archive a card
    Archive {
        /// Card ID
        id: String,
    },
    /// Unarchive a card
    Unarchive {
        /// Card ID
        id: String,
    },
    /// Delete a card
    Delete {
        /// Card ID
        id: String,
    },
    /// Manage labels on a card
    Label(CardLabelCommand),
    /// Manage assignees on a card
    Assignee(CardAssigneeCommand),
}

// ── Card Label ──────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct CardLabelCommand {
    #[command(subcommand)]
    pub action: CardLabelAction,
}

#[derive(Subcommand)]
pub enum CardLabelAction {
    /// List labels on a card
    List {
        /// Card ID
        card: String,
    },
    /// Add a label to a card
    Add {
        /// Card ID
        card: String,
        /// Label ID
        label: String,
    },
    /// Remove a label from a card
    Remove {
        /// Card ID
        card: String,
        /// Label ID
        label: String,
    },
}

// ── Card Assignee ───────────────────────────────────────────────────────

#[derive(Parser)]
pub struct CardAssigneeCommand {
    #[command(subcommand)]
    pub action: CardAssigneeAction,
}

#[derive(Subcommand)]
pub enum CardAssigneeAction {
    /// List assignees on a card
    List {
        /// Card ID
        card: String,
    },
    /// Add an assignee to a card
    Add {
        /// Card ID
        card: String,
        /// User ID
        user: String,
    },
    /// Remove an assignee from a card
    Remove {
        /// Card ID
        card: String,
        /// User ID
        user: String,
    },
}

// ── Task ────────────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct TaskCommand {
    #[command(subcommand)]
    pub action: TaskAction,
}

#[derive(Subcommand)]
pub enum TaskAction {
    /// List tasks on a card
    List {
        /// Parent card ID
        #[arg(long)]
        card: String,
    },
    /// Create a new task
    Create {
        /// Parent card ID
        #[arg(long)]
        card: String,
        /// Task title
        #[arg(long)]
        title: String,
    },
    /// Update a task
    Update {
        /// Task ID
        id: String,
        /// New task title
        #[arg(long)]
        title: Option<String>,
    },
    /// Mark a task as completed
    Complete {
        /// Task ID
        id: String,
    },
    /// Reopen a completed task
    Reopen {
        /// Task ID
        id: String,
    },
    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },
}

// ── Comment ─────────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct CommentCommand {
    #[command(subcommand)]
    pub action: CommentAction,
}

#[derive(Subcommand)]
pub enum CommentAction {
    /// List comments on a card
    List {
        /// Parent card ID
        #[arg(long)]
        card: String,
    },
    /// Create a new comment
    Create {
        /// Parent card ID
        #[arg(long)]
        card: String,
        /// Comment text (literal, "-" for stdin, "@file" for file)
        #[arg(long)]
        text: String,
    },
    /// Update a comment
    Update {
        /// Comment ID
        id: String,
        /// New comment text (literal, "-" for stdin, "@file" for file)
        #[arg(long)]
        text: String,
    },
    /// Delete a comment
    Delete {
        /// Comment ID
        id: String,
    },
}

// ── Label ───────────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct LabelCommand {
    #[command(subcommand)]
    pub action: LabelAction,
}

#[derive(Subcommand)]
pub enum LabelAction {
    /// List labels on a board
    List {
        /// Parent board ID
        #[arg(long)]
        board: String,
    },
    /// Find labels by name within a board
    Find {
        /// Parent board ID
        #[arg(long)]
        board: String,
        /// Label name to search for
        #[arg(long)]
        name: String,
    },
    /// Create a new label
    Create {
        /// Parent board ID
        #[arg(long)]
        board: String,
        /// Label name
        #[arg(long)]
        name: String,
        /// Label color (e.g., berry-red, pumpkin-orange, rain-blue)
        #[arg(long)]
        color: String,
    },
    /// Update a label
    Update {
        /// Label ID
        id: String,
        /// New label name
        #[arg(long)]
        name: Option<String>,
        /// New label color
        #[arg(long)]
        color: Option<String>,
    },
    /// Delete a label
    Delete {
        /// Label ID
        id: String,
    },
}

// ── Attachment ──────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct AttachmentCommand {
    #[command(subcommand)]
    pub action: AttachmentAction,
}

#[derive(Subcommand)]
pub enum AttachmentAction {
    /// List attachments on a card
    List {
        /// Parent card ID
        #[arg(long)]
        card: String,
    },
    /// Upload a file to a card
    Upload {
        /// Parent card ID
        #[arg(long)]
        card: String,
        /// File path to upload
        file: String,
    },
    /// Download an attachment to a local file
    Download {
        /// Attachment ID
        id: String,
        /// Parent card ID (used to resolve the real filename)
        #[arg(long)]
        card: String,
        /// Output file path (defaults to attachment's original filename)
        #[arg(long)]
        out: Option<String>,
    },
    /// Delete an attachment
    Delete {
        /// Attachment ID
        id: String,
    },
}

// ── Membership ──────────────────────────────────────────────────────────

#[derive(Parser)]
pub struct MembershipCommand {
    #[command(subcommand)]
    pub action: MembershipAction,
}

#[derive(Subcommand)]
pub enum MembershipAction {
    /// List members of a project or board
    List {
        /// Project ID (mutually exclusive with --board)
        #[arg(long)]
        project: Option<String>,
        /// Board ID (mutually exclusive with --project)
        #[arg(long)]
        board: Option<String>,
    },
    /// Add a member to a project or board
    Add {
        /// Project ID (mutually exclusive with --board)
        #[arg(long)]
        project: Option<String>,
        /// Board ID (mutually exclusive with --project)
        #[arg(long)]
        board: Option<String>,
        /// User ID to add
        #[arg(long)]
        user: String,
        /// Role (e.g., editor, viewer)
        #[arg(long)]
        role: Option<String>,
    },
    /// Remove a member from a project or board
    Remove {
        /// Project ID (mutually exclusive with --board)
        #[arg(long)]
        project: Option<String>,
        /// Board ID (mutually exclusive with --project)
        #[arg(long)]
        board: Option<String>,
        /// User ID to remove
        #[arg(long)]
        user: String,
    },
}
