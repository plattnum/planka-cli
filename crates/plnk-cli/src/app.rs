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
}

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
    /// List cards in a list
    List {
        /// Parent list ID
        #[arg(long)]
        list: String,
    },
    /// Get a card by ID
    Get {
        /// Card ID
        id: String,
    },
    /// Find cards by title within a scope
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
        title: String,
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
    /// Move a card to a different list
    Move {
        /// Card ID
        id: String,
        /// Target list ID
        #[arg(long)]
        to_list: String,
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
    /// Get a task by ID
    Get {
        /// Task ID
        id: String,
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
    /// Get a comment by ID
    Get {
        /// Comment ID
        id: String,
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
    /// Get a label by ID
    Get {
        /// Label ID
        id: String,
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
        /// Output file path
        #[arg(long)]
        out: String,
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
