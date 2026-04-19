use std::collections::HashSet;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::{Sink, SinkExt, StreamExt};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use url::Url;

const SOCKET_IO_PROTOCOL_VERSION: &str = "4";
const SAILS_IO_SDK_VERSION: &str = "1.2.1";
const SUBSCRIBE_ACK_ID: u64 = 1;

#[derive(Debug, Parser)]
#[command(
    name = "plnk-tui",
    version,
    about = "Experimental TUI websocket spike for Planka"
)]
struct Args {
    /// Planka server URL
    #[arg(long, env = "PLANKA_SERVER")]
    server: Option<String>,

    /// Username or email for interactive login
    #[arg(long, env = "PLANKA_USERNAME")]
    username: Option<String>,

    /// Password for interactive login (omit to prompt securely)
    #[arg(long, env = "PLANKA_PASSWORD")]
    password: Option<String>,

    /// Board ID to subscribe to for live updates
    #[arg(long, env = "PLNK_TUI_BOARD")]
    board: String,

    /// Hidden debug mode: print socket events to stdout without the TUI
    #[arg(long, hide = true)]
    headless: bool,

    /// Hidden debug timeout for --headless
    #[arg(long, hide = true, default_value_t = 10)]
    headless_timeout_secs: u64,
}

#[derive(Debug, Error)]
enum TuiError {
    #[error("--server or PLANKA_SERVER is required")]
    MissingServer,
    #[error("--username or PLANKA_USERNAME is required")]
    MissingUsername,
    #[error("terminal error: {0}")]
    Io(#[from] io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("url error: {0}")]
    Url(#[from] url::ParseError),
    #[error("authentication failed: {0}")]
    Authentication(String),
    #[error("socket error: {0}")]
    Socket(String),
    #[error("unexpected response from server: {0}")]
    UnexpectedResponse(String),
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectsResponse {
    items: Vec<ProjectSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSummary {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ItemResponse<T> {
    item: T,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurrentUser {
    id: String,
    username: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TermsDocument {
    signature: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TermsRequiredResponse {
    pending_token: String,
    step: String,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocketResponse<T> {
    body: T,
    status_code: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineOpen {
    sid: String,
    ping_interval: u64,
    ping_timeout: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoardSnapshot {
    item: BoardItem,
    included: BoardIncluded,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoardItem {
    id: String,
    name: String,
    project_id: String,
    position: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoardIncluded {
    lists: Vec<BoardListItem>,
    cards: Vec<CardItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoardListItem {
    id: String,
    name: Option<String>,
    position: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardItem {
    id: String,
    list_id: String,
    name: String,
    position: f64,
    is_closed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSnapshot {
    item: ProjectSummary,
    included: ProjectIncluded,
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectIncluded {
    boards: Vec<ProjectBoardItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectBoardItem {
    id: String,
    name: String,
    position: f64,
}

#[derive(Debug, Clone)]
struct CardSummary {
    id: String,
    name: String,
}

#[derive(Debug, Clone)]
struct ListSummary {
    id: String,
    name: String,
    card_count: usize,
    cards: Vec<CardSummary>,
}

#[derive(Debug, Clone)]
struct BoardSummary {
    id: String,
    name: String,
    project_id: String,
    position: f64,
    total_cards: usize,
    active_lists: Vec<ListSummary>,
}

#[derive(Debug, Clone)]
struct ProjectTree {
    id: String,
    name: String,
    boards: Vec<BoardSummary>,
}

impl BoardSummary {
    fn from_snapshot(snapshot: BoardSnapshot) -> Self {
        let mut cards = snapshot
            .included
            .cards
            .into_iter()
            .filter(|card| !card.is_closed)
            .collect::<Vec<_>>();
        cards.sort_by(|left, right| left.position.total_cmp(&right.position));

        let total_cards = cards.len();

        let mut lists = snapshot.included.lists;
        lists.sort_by(|left, right| match (left.position, right.position) {
            (Some(left), Some(right)) => left.total_cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        let active_lists = lists
            .into_iter()
            .filter_map(|list| {
                let name = list.name?;
                let _position = list.position?;
                if name.trim().is_empty() {
                    return None;
                }

                let list_cards = cards
                    .iter()
                    .filter(|card| card.list_id == list.id)
                    .map(|card| CardSummary {
                        id: card.id.clone(),
                        name: card.name.clone(),
                    })
                    .collect::<Vec<_>>();

                Some(ListSummary {
                    id: list.id,
                    name,
                    card_count: list_cards.len(),
                    cards: list_cards,
                })
            })
            .collect::<Vec<_>>();

        Self {
            id: snapshot.item.id,
            name: snapshot.item.name,
            project_id: snapshot.item.project_id,
            position: snapshot.item.position,
            total_cards,
            active_lists,
        }
    }

    fn stub(project_id: String, id: String, name: String, position: f64) -> Self {
        Self {
            id,
            name,
            project_id,
            position,
            total_cards: 0,
            active_lists: Vec::new(),
        }
    }

    fn is_loaded(&self) -> bool {
        !self.active_lists.is_empty() || self.total_cards > 0
    }
}

#[derive(Debug, Clone)]
struct LiveEventRecord {
    name: String,
    summary: String,
}

#[derive(Debug, Clone)]
enum AppEvent {
    SocketConnecting,
    SocketLive(BoardSummary),
    SocketError(String),
    LiveEvent(LiveEventRecord),
}

#[derive(Debug, Clone)]
enum ConnectionState {
    Loading,
    Connecting,
    Live,
    Error(String),
}

impl ConnectionState {
    fn label(&self) -> String {
        match self {
            Self::Loading => "loading".to_string(),
            Self::Connecting => "connecting raw websocket".to_string(),
            Self::Live => "live websocket connected".to_string(),
            Self::Error(message) => format!("error: {message}"),
        }
    }

    fn style(&self) -> Style {
        match self {
            Self::Loading => Style::default().fg(Color::Yellow),
            Self::Connecting => Style::default().fg(Color::LightYellow),
            Self::Live => Style::default().fg(Color::LightGreen),
            Self::Error(_) => Style::default().fg(Color::LightRed),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TreeKey {
    Project(String),
    Board(String),
    List(String),
    Card(String),
}

#[derive(Debug, Clone, Copy)]
enum TreeKind {
    Project,
    Board,
    List,
    Card,
}

#[derive(Debug, Clone)]
struct TreeRow {
    key: TreeKey,
    parent: Option<TreeKey>,
    depth: usize,
    kind: TreeKind,
    label: String,
    meta: Option<String>,
    has_children: bool,
    expanded: bool,
    live: bool,
}

#[derive(Debug, Clone)]
struct AppState {
    server: String,
    login: String,
    current_user: CurrentUser,
    projects: Vec<ProjectTree>,
    board: Option<BoardSummary>,
    subscribed_board_id: String,
    status: ConnectionState,
    recent_events: Vec<LiveEventRecord>,
    expanded_projects: HashSet<String>,
    expanded_boards: HashSet<String>,
    expanded_lists: HashSet<String>,
    selected: Option<TreeKey>,
    show_debug_log: bool,
}

#[derive(Debug, Default)]
struct SocketSessionState {
    engine_sid: Option<String>,
    namespace_connected: bool,
    subscribe_sent: bool,
}

impl AppState {
    fn new(
        server: String,
        login: String,
        current_user: CurrentUser,
        projects: Vec<ProjectTree>,
        subscribed_board_id: String,
    ) -> Self {
        let mut expanded_projects = HashSet::new();
        let mut expanded_boards = HashSet::new();
        let expanded_lists = HashSet::new();

        let selected = projects
            .iter()
            .find(|project| {
                project
                    .boards
                    .iter()
                    .any(|board| board.id == subscribed_board_id)
            })
            .map(|project| {
                expanded_projects.insert(project.id.clone());
                expanded_boards.insert(subscribed_board_id.clone());
                TreeKey::Board(subscribed_board_id.clone())
            })
            .or_else(|| {
                projects
                    .first()
                    .map(|project| TreeKey::Project(project.id.clone()))
            });

        Self {
            server,
            login,
            current_user,
            projects,
            board: None,
            subscribed_board_id,
            status: ConnectionState::Loading,
            recent_events: Vec::new(),
            expanded_projects,
            expanded_boards,
            expanded_lists,
            selected,
            show_debug_log: false,
        }
    }

    fn apply(&mut self, event: AppEvent) {
        match event {
            AppEvent::SocketConnecting => {
                self.status = ConnectionState::Connecting;
            }
            AppEvent::SocketLive(board) => {
                self.merge_board(&board);
                self.board = Some(board);
                self.status = ConnectionState::Live;
            }
            AppEvent::SocketError(message) => {
                self.status = ConnectionState::Error(message);
            }
            AppEvent::LiveEvent(record) => {
                self.recent_events.insert(0, record);
                self.recent_events.truncate(24);
            }
        }
    }

    fn merge_board(&mut self, board: &BoardSummary) {
        let mut expanded_any_list = false;
        for project in &mut self.projects {
            if project.id != board.project_id {
                continue;
            }

            if let Some(existing) = project.boards.iter_mut().find(|item| item.id == board.id) {
                *existing = board.clone();
            } else {
                project.boards.push(board.clone());
                project
                    .boards
                    .sort_by(|left, right| left.position.total_cmp(&right.position));
            }

            if board.id == self.subscribed_board_id {
                self.expanded_projects.insert(project.id.clone());
                self.expanded_boards.insert(board.id.clone());
                expanded_any_list = board
                    .active_lists
                    .iter()
                    .any(|list| self.expanded_lists.contains(&list.id));
            }
        }

        if board.id == self.subscribed_board_id && !expanded_any_list {
            self.expanded_lists
                .extend(board.active_lists.iter().map(|list| list.id.clone()));
        }
    }

    fn visible_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();

        for project in &self.projects {
            let project_expanded = self.expanded_projects.contains(&project.id);
            rows.push(TreeRow {
                key: TreeKey::Project(project.id.clone()),
                parent: None,
                depth: 0,
                kind: TreeKind::Project,
                label: project.name.clone(),
                meta: Some(format!("{} boards", project.boards.len())),
                has_children: !project.boards.is_empty(),
                expanded: project_expanded,
                live: false,
            });

            if !project_expanded {
                continue;
            }

            for board in &project.boards {
                let board_expanded = self.expanded_boards.contains(&board.id);
                let board_loaded = board.is_loaded();
                let board_meta = if board.id == self.subscribed_board_id && !board_loaded {
                    Some("syncing live snapshot…".to_string())
                } else if board_loaded {
                    Some(format!(
                        "{} lists • {} cards",
                        board.active_lists.len(),
                        board.total_cards
                    ))
                } else {
                    Some("board discovered".to_string())
                };

                rows.push(TreeRow {
                    key: TreeKey::Board(board.id.clone()),
                    parent: Some(TreeKey::Project(project.id.clone())),
                    depth: 1,
                    kind: TreeKind::Board,
                    label: board.name.clone(),
                    meta: board_meta,
                    has_children: board_loaded && !board.active_lists.is_empty(),
                    expanded: board_expanded,
                    live: board.id == self.subscribed_board_id,
                });

                if !(board_expanded && board_loaded) {
                    continue;
                }

                for list in &board.active_lists {
                    let list_expanded = self.expanded_lists.contains(&list.id);
                    rows.push(TreeRow {
                        key: TreeKey::List(list.id.clone()),
                        parent: Some(TreeKey::Board(board.id.clone())),
                        depth: 2,
                        kind: TreeKind::List,
                        label: list.name.clone(),
                        meta: Some(format!("{} cards", list.card_count)),
                        has_children: !list.cards.is_empty(),
                        expanded: list_expanded,
                        live: false,
                    });

                    if !list_expanded {
                        continue;
                    }

                    for card in &list.cards {
                        rows.push(TreeRow {
                            key: TreeKey::Card(card.id.clone()),
                            parent: Some(TreeKey::List(list.id.clone())),
                            depth: 3,
                            kind: TreeKind::Card,
                            label: card.name.clone(),
                            meta: None,
                            has_children: false,
                            expanded: false,
                            live: false,
                        });
                    }
                }
            }
        }

        rows
    }

    fn selected_index(&self, rows: &[TreeRow]) -> usize {
        rows.iter()
            .position(|row| {
                self.selected
                    .as_ref()
                    .is_some_and(|selected| selected == &row.key)
            })
            .unwrap_or(0)
    }

    fn select_relative(&mut self, delta: isize) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }

        let current = self.selected_index(&rows);
        let last = rows.len().saturating_sub(1);
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta.unsigned_abs()).min(last)
        };

        self.selected = Some(rows[next].key.clone());
    }

    fn expand_or_descend(&mut self) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }

        let index = self.selected_index(&rows);
        let row = &rows[index];

        match &row.key {
            TreeKey::Project(project_id) => {
                if row.has_children && !row.expanded {
                    self.expanded_projects.insert(project_id.clone());
                } else if let Some(next_row) = rows.get(index + 1) {
                    self.selected = Some(next_row.key.clone());
                }
            }
            TreeKey::Board(board_id) => {
                if row.has_children && !row.expanded {
                    self.expanded_boards.insert(board_id.clone());
                } else if let Some(next_row) = rows.get(index + 1) {
                    self.selected = Some(next_row.key.clone());
                }
            }
            TreeKey::List(list_id) => {
                if row.has_children && !row.expanded {
                    self.expanded_lists.insert(list_id.clone());
                } else if let Some(next_row) = rows.get(index + 1) {
                    self.selected = Some(next_row.key.clone());
                }
            }
            TreeKey::Card(_) => {}
        }
    }

    fn collapse_or_ascend(&mut self) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }

        let index = self.selected_index(&rows);
        let row = &rows[index];

        match &row.key {
            TreeKey::Project(project_id) => {
                self.expanded_projects.remove(project_id);
            }
            TreeKey::Board(board_id) => {
                if row.expanded {
                    self.expanded_boards.remove(board_id);
                } else if let Some(parent) = &row.parent {
                    self.selected = Some(parent.clone());
                }
            }
            TreeKey::List(list_id) => {
                if row.expanded {
                    self.expanded_lists.remove(list_id);
                } else if let Some(parent) = &row.parent {
                    self.selected = Some(parent.clone());
                }
            }
            TreeKey::Card(_) => {
                if let Some(parent) = &row.parent {
                    self.selected = Some(parent.clone());
                }
            }
        }
    }

    fn toggle_debug_log(&mut self) {
        self.show_debug_log = !self.show_debug_log;
    }

    fn latest_event_summary(&self) -> String {
        self.recent_events.first().map_or_else(
            || "waiting for live websocket events".to_string(),
            |record| format!("{} :: {}", record.name, record.summary),
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest<'a> {
    email_or_username: &'a str,
    password: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptTermsRequest<'a> {
    pending_token: &'a str,
    signature: &'a str,
    initial_language: &'a str,
}

#[tokio::main]
async fn main() -> Result<(), TuiError> {
    let args = Args::parse();
    let server = Url::parse(args.server.as_deref().ok_or(TuiError::MissingServer)?)?;
    let username = args.username.ok_or(TuiError::MissingUsername)?;
    let password = match args.password {
        Some(password) => password,
        None => rpassword::prompt_password("Planka password: ")?,
    };

    let token = authenticate(&server, &username, &password).await?;
    let current_user = fetch_current_user(&server, &token).await?;
    let projects = fetch_projects(&server, &token).await?;
    let project_trees = fetch_project_trees(&server, &token, &projects).await?;

    let (tx, rx) = mpsc::channel();
    spawn_socket_listener(server.clone(), token, args.board.clone(), tx.clone());

    if args.headless {
        run_headless_probe(&current_user, &projects, &rx, args.headless_timeout_secs);
        return Ok(());
    }

    let mut terminal = init_terminal()?;
    let mut app = AppState::new(
        server.to_string(),
        username,
        current_user,
        project_trees,
        args.board,
    );

    let result = run_app(&mut terminal, &mut app, &rx);
    restore_terminal()?;
    result
}

async fn authenticate(server: &Url, username: &str, password: &str) -> Result<String, TuiError> {
    let client = reqwest::Client::new();
    let login_url = server.join("api/access-tokens")?;
    let response = client
        .post(login_url)
        .json(&LoginRequest {
            email_or_username: username,
            password,
        })
        .send()
        .await?;

    if response.status().is_success() {
        let token = response.json::<ItemResponse<String>>().await?;
        return Ok(token.item);
    }

    if response.status() == StatusCode::FORBIDDEN {
        let terms_required = response.json::<TermsRequiredResponse>().await?;
        if terms_required.step == "accept-terms" {
            let terms = client
                .get(server.join("api/terms?language=en-US")?)
                .send()
                .await?
                .json::<ItemResponse<TermsDocument>>()
                .await?;

            let accepted = client
                .post(server.join("api/access-tokens/accept-terms")?)
                .json(&AcceptTermsRequest {
                    pending_token: &terms_required.pending_token,
                    signature: &terms.item.signature,
                    initial_language: "en-US",
                })
                .send()
                .await?;

            if accepted.status().is_success() {
                let token = accepted.json::<ItemResponse<String>>().await?;
                return Ok(token.item);
            }

            return Err(TuiError::Authentication(
                accepted
                    .text()
                    .await
                    .unwrap_or_else(|_| "terms acceptance failed".to_string()),
            ));
        }

        return Err(TuiError::Authentication(terms_required.message));
    }

    Err(TuiError::Authentication(
        response
            .text()
            .await
            .unwrap_or_else(|_| "login failed".to_string()),
    ))
}

async fn fetch_current_user(server: &Url, token: &str) -> Result<CurrentUser, TuiError> {
    let client = reqwest::Client::new();
    let response = client
        .get(server.join("api/users/me")?)
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<ItemResponse<CurrentUser>>().await?.item)
}

async fn fetch_projects(server: &Url, token: &str) -> Result<Vec<ProjectSummary>, TuiError> {
    let client = reqwest::Client::new();
    let response = client
        .get(server.join("api/projects")?)
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<ProjectsResponse>().await?.items)
}

async fn fetch_project_trees(
    server: &Url,
    token: &str,
    projects: &[ProjectSummary],
) -> Result<Vec<ProjectTree>, TuiError> {
    let client = reqwest::Client::new();
    let mut trees = Vec::with_capacity(projects.len());

    for project in projects {
        let response = client
            .get(server.join(&format!("api/projects/{}", project.id))?)
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?;

        let snapshot = response.json::<ProjectSnapshot>().await?;
        let mut boards = snapshot
            .included
            .boards
            .into_iter()
            .map(|board| {
                BoardSummary::stub(
                    snapshot.item.id.clone(),
                    board.id,
                    board.name,
                    board.position,
                )
            })
            .collect::<Vec<_>>();
        boards.sort_by(|left, right| left.position.total_cmp(&right.position));

        trees.push(ProjectTree {
            id: snapshot.item.id,
            name: snapshot.item.name,
            boards,
        });
    }

    Ok(trees)
}

fn spawn_socket_listener(server: Url, token: String, board_id: String, tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        if let Err(err) = socket_task(server, token, board_id, tx.clone()).await {
            let _ = tx.send(AppEvent::SocketError(err.to_string()));
        }
    });
}

async fn socket_task(
    server: Url,
    token: String,
    board_id: String,
    tx: Sender<AppEvent>,
) -> Result<(), TuiError> {
    let _ = tx.send(AppEvent::SocketConnecting);

    let request = build_socket_request(&server)?;
    let (mut socket, _response) = connect_async(request)
        .await
        .map_err(|err| TuiError::Socket(format!("websocket connect failed: {err}")))?;

    let mut state = SocketSessionState::default();

    while let Some(message) = socket.next().await {
        let message =
            message.map_err(|err| TuiError::Socket(format!("websocket read failed: {err}")))?;

        match message {
            Message::Text(text) => {
                handle_engine_text_message(&text, &mut socket, &token, &board_id, &tx, &mut state)
                    .await?;
            }
            Message::Ping(payload) => {
                socket
                    .send(Message::Pong(payload))
                    .await
                    .map_err(|err| TuiError::Socket(format!("websocket pong failed: {err}")))?;
            }
            Message::Close(frame) => {
                let detail = frame.map_or_else(
                    || "close frame missing".to_string(),
                    |value| value.to_string(),
                );
                return Err(TuiError::Socket(format!("websocket closed: {detail}")));
            }
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }

    Err(TuiError::Socket(
        "websocket stream ended unexpectedly".to_string(),
    ))
}

async fn handle_engine_text_message<S>(
    text: &str,
    socket: &mut S,
    token: &str,
    board_id: &str,
    tx: &Sender<AppEvent>,
    state: &mut SocketSessionState,
) -> Result<(), TuiError>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let Some(engine_type) = text.chars().next() else {
        return Ok(());
    };

    match engine_type {
        '0' => {
            let open = serde_json::from_str::<EngineOpen>(&text[1..])
                .map_err(|err| TuiError::Socket(format!("invalid engine open packet: {err}")))?;
            state.engine_sid = Some(open.sid.clone());
            let _ = tx.send(AppEvent::LiveEvent(LiveEventRecord {
                name: "engineOpen".to_string(),
                summary: format!(
                    "sid={} pingInterval={} pingTimeout={}",
                    open.sid, open.ping_interval, open.ping_timeout
                ),
            }));
            socket
                .send(Message::Text("40".to_string()))
                .await
                .map_err(|err| TuiError::Socket(format!("socket connect packet failed: {err}")))?;
        }
        '1' => {
            return Err(TuiError::Socket(
                "server sent engine close packet".to_string(),
            ));
        }
        '2' => {
            socket
                .send(Message::Text("3".to_string()))
                .await
                .map_err(|err| TuiError::Socket(format!("engine pong failed: {err}")))?;
        }
        '3' | '5' | '6' => {}
        '4' => {
            let socket_packet = &text[1..];
            handle_socket_io_packet(
                socket_packet,
                socket,
                token,
                board_id,
                tx,
                &mut state.namespace_connected,
                &mut state.subscribe_sent,
            )
            .await?;
        }
        other => {
            let _ = tx.send(AppEvent::LiveEvent(LiveEventRecord {
                name: "engineOther".to_string(),
                summary: format!("type={other} raw={}", truncate(text, 120)),
            }));
        }
    }

    Ok(())
}

async fn handle_socket_io_packet<S>(
    packet: &str,
    socket: &mut S,
    token: &str,
    board_id: &str,
    tx: &Sender<AppEvent>,
    namespace_connected: &mut bool,
    subscribe_sent: &mut bool,
) -> Result<(), TuiError>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    if packet.starts_with('0') {
        *namespace_connected = true;
        let _ = tx.send(AppEvent::LiveEvent(LiveEventRecord {
            name: "socketConnect".to_string(),
            summary: truncate(packet, 120),
        }));

        if !*subscribe_sent {
            let subscribe = build_subscribe_packet(token, board_id)?;
            socket
                .send(Message::Text(subscribe))
                .await
                .map_err(|err| TuiError::Socket(format!("board subscribe send failed: {err}")))?;
            *subscribe_sent = true;
        }
        return Ok(());
    }

    if packet.starts_with('4') {
        return Err(TuiError::Socket(format!(
            "socket connect error packet: {}",
            truncate(packet, 200)
        )));
    }

    if let Some((ack_id, ack_value)) = parse_socket_ack(packet)? {
        if ack_id == SUBSCRIBE_ACK_ID {
            let board = parse_board_snapshot_value(ack_value)?;
            let _ = tx.send(AppEvent::SocketLive(board));
        }
        return Ok(());
    }

    if let Some((event_name, payload)) = parse_socket_event(packet)? {
        let _ = tx.send(AppEvent::LiveEvent(LiveEventRecord {
            name: event_name,
            summary: summarize_json(&payload),
        }));
        return Ok(());
    }

    let _ = tx.send(AppEvent::LiveEvent(LiveEventRecord {
        name: "socketPacket".to_string(),
        summary: truncate(packet, 160),
    }));

    Ok(())
}

fn build_socket_request(
    server: &Url,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>, TuiError> {
    let mut url = server.clone();
    url.set_path("/socket.io/");
    url.set_query(None);
    url.query_pairs_mut()
        .append_pair("EIO", SOCKET_IO_PROTOCOL_VERSION)
        .append_pair("transport", "websocket")
        .append_pair("__sails_io_sdk_version", SAILS_IO_SDK_VERSION)
        .append_pair("__sails_io_sdk_platform", "node")
        .append_pair("__sails_io_sdk_language", "javascript");

    match url.scheme() {
        "http" => {
            url.set_scheme("ws")
                .map_err(|()| TuiError::Socket("failed to convert http URL to ws".to_string()))?;
        }
        "https" => {
            url.set_scheme("wss")
                .map_err(|()| TuiError::Socket("failed to convert https URL to wss".to_string()))?;
        }
        "ws" | "wss" => {}
        other => {
            return Err(TuiError::Socket(format!(
                "unsupported websocket URL scheme: {other}"
            )));
        }
    }

    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|err| TuiError::Socket(format!("invalid websocket request: {err}")))?;

    let origin = origin_from_url(server);
    let origin_header = HeaderValue::from_str(&origin)
        .map_err(|err| TuiError::Socket(format!("invalid origin header: {err}")))?;
    request.headers_mut().insert("Origin", origin_header);

    Ok(request)
}

fn origin_from_url(server: &Url) -> String {
    match server.port() {
        Some(port) => format!(
            "{}://{}:{}",
            server.scheme(),
            server.host_str().unwrap_or_default(),
            port
        ),
        None => format!(
            "{}://{}",
            server.scheme(),
            server.host_str().unwrap_or_default()
        ),
    }
}

fn build_subscribe_packet(token: &str, board_id: &str) -> Result<String, TuiError> {
    let payload = json!([
        "get",
        {
            "method": "get",
            "url": format!("/api/boards/{board_id}?subscribe=true"),
            "data": {},
            "headers": {
                "Authorization": format!("Bearer {token}")
            }
        }
    ]);

    let encoded = serde_json::to_string(&payload)
        .map_err(|err| TuiError::Socket(format!("subscribe payload encode failed: {err}")))?;
    Ok(format!("42{SUBSCRIBE_ACK_ID}{encoded}"))
}

fn parse_socket_ack(packet: &str) -> Result<Option<(u64, Value)>, TuiError> {
    if !packet.starts_with('3') {
        return Ok(None);
    }

    let rest = &packet[1..];
    let Some(payload_start) = rest.find('[') else {
        return Err(TuiError::Socket(format!(
            "ack packet missing payload: {}",
            truncate(packet, 160)
        )));
    };

    let ack_id = rest[..payload_start].parse::<u64>().map_err(|err| {
        TuiError::Socket(format!(
            "ack packet id parse failed: {err}: {}",
            truncate(packet, 160)
        ))
    })?;

    let payload = serde_json::from_str::<Vec<Value>>(&rest[payload_start..]).map_err(|err| {
        TuiError::Socket(format!(
            "ack packet payload parse failed: {err}: {}",
            truncate(packet, 160)
        ))
    })?;

    Ok(payload.into_iter().next().map(|value| (ack_id, value)))
}

fn parse_socket_event(packet: &str) -> Result<Option<(String, Value)>, TuiError> {
    if !packet.starts_with('2') {
        return Ok(None);
    }

    let rest = &packet[1..];
    if !rest.starts_with('[') {
        return Ok(None);
    }

    let payload = serde_json::from_str::<Vec<Value>>(rest).map_err(|err| {
        TuiError::Socket(format!(
            "event packet payload parse failed: {err}: {}",
            truncate(packet, 160)
        ))
    })?;

    let Some(name) = payload.first().and_then(Value::as_str) else {
        return Ok(None);
    };

    let value = payload.get(1).cloned().unwrap_or(Value::Null);
    Ok(Some((name.to_string(), value)))
}

fn parse_board_snapshot_value(value: Value) -> Result<BoardSummary, TuiError> {
    let response = serde_json::from_value::<SocketResponse<BoardSnapshot>>(value)
        .map_err(|err| TuiError::UnexpectedResponse(err.to_string()))?;

    if response.status_code != 200 {
        return Err(TuiError::UnexpectedResponse(format!(
            "board subscribe returned status {}",
            response.status_code
        )));
    }

    Ok(BoardSummary::from_snapshot(response.body))
}

fn summarize_json(value: &Value) -> String {
    if let Some(item) = value.get("item") {
        if let Some(name) = item.get("name").and_then(Value::as_str) {
            let id = item.get("id").and_then(Value::as_str).unwrap_or("?");
            return truncate(&format!("{name} [{id}]"), 120);
        }
        if let Some(id) = item.get("id").and_then(Value::as_str) {
            return truncate(&format!("item [{id}]"), 120);
        }
    }

    if let Some(name) = value.get("name").and_then(Value::as_str) {
        return truncate(name, 120);
    }

    truncate(&value.to_string(), 120)
}

fn truncate(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, TuiError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout)).map_err(TuiError::Io)
}

fn restore_terminal() -> Result<(), TuiError> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen)?;
    Ok(())
}

fn run_headless_probe(
    current_user: &CurrentUser,
    projects: &[ProjectSummary],
    rx: &Receiver<AppEvent>,
    timeout_secs: u64,
) {
    println!(
        "headless probe: current_user={} ({}) visible_projects={}",
        current_user.name,
        current_user.username,
        projects.len()
    );

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(timeout_secs) {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(AppEvent::SocketConnecting) => println!("event: socket connecting"),
            Ok(AppEvent::SocketLive(board)) => println!(
                "event: board live: {} [{}] cards={} lists={}",
                board.name,
                board.id,
                board.total_cards,
                board.active_lists.len()
            ),
            Ok(AppEvent::SocketError(message)) => println!("event: socket error: {message}"),
            Ok(AppEvent::LiveEvent(record)) => {
                println!("event: {} :: {}", record.name, record.summary);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
    rx: &Receiver<AppEvent>,
) -> Result<(), TuiError> {
    loop {
        while let Ok(message) = rx.try_recv() {
            app.apply(message);
        }

        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                CEvent::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => app.select_relative(1),
                    KeyCode::Up | KeyCode::Char('k') => app.select_relative(-1),
                    KeyCode::Left | KeyCode::Char('h') => app.collapse_or_ascend(),
                    KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                        app.expand_or_descend();
                    }
                    KeyCode::Char('d' | 'D') => app.toggle_debug_log(),
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn draw(frame: &mut ratatui::Frame<'_>, app: &AppState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(area);

    let header_lines = vec![
        Line::from(vec![
            Span::styled(
                "plnk-tui explorer",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  •  "),
            Span::styled(app.status.label(), app.status.style()),
        ]),
        Line::from(format!(
            "server: {} | login: {} | current user: {} ({})",
            app.server, app.login, app.current_user.name, app.current_user.username
        )),
        Line::from(format!(
            "visible projects: {} | current user id: {} | subscribed board: {}",
            app.projects.len(),
            app.current_user.id,
            app.subscribed_board_id
        )),
    ];
    frame.render_widget(
        Paragraph::new(header_lines)
            .block(panel_block("session"))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
        .split(chunks[1]);

    let rows = app.visible_rows();
    let selected_index = app.selected_index(&rows);

    let tree_items = if rows.is_empty() {
        vec![ListItem::new("No projects visible for this user.")]
    } else {
        rows.iter().map(render_tree_row).collect::<Vec<_>>()
    };

    let mut tree_state = ListState::default();
    if !rows.is_empty() {
        tree_state.select(Some(selected_index));
    }

    let tree = List::new(tree_items)
        .block(panel_block("explorer"))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(32, 46, 70))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌ ");
    frame.render_stateful_widget(tree, body[0], &mut tree_state);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(7)])
        .split(body[1]);

    let detail_lines = build_detail_lines(app);
    frame.render_widget(
        Paragraph::new(detail_lines)
            .block(panel_block("details"))
            .wrap(Wrap { trim: true }),
        right[0],
    );

    let live_lines = vec![
        Line::from(vec![
            Span::styled("websocket", Style::default().fg(Color::LightCyan)),
            Span::raw(": "),
            Span::styled(app.status.label(), app.status.style()),
        ]),
        Line::from(format!(
            "subscribed board: {}",
            app.board.as_ref().map_or_else(
                || format!("{} (waiting for snapshot)", app.subscribed_board_id),
                |board| format!("{} [{}]", board.name, board.id),
            )
        )),
        Line::from(format!("latest event: {}", app.latest_event_summary())),
        Line::from(""),
        Line::from("tip: press D to toggle the websocket debug log overlay"),
    ];
    frame.render_widget(
        Paragraph::new(live_lines)
            .block(panel_block("live sync"))
            .wrap(Wrap { trim: true }),
        right[1],
    );

    frame.render_widget(
        Paragraph::new(
            "j/k move • h/l collapse or expand • Enter descend • D debug log • q / Esc quit",
        )
        .block(panel_block("keys")),
        chunks[2],
    );

    if app.show_debug_log {
        draw_debug_overlay(frame, area, app);
    }
}

fn render_tree_row(row: &TreeRow) -> ListItem<'static> {
    let indent = "  ".repeat(row.depth);
    let branch = if row.has_children {
        if row.expanded { "▾" } else { "▸" }
    } else {
        "•"
    };
    let icon = match row.kind {
        TreeKind::Project => "◼",
        TreeKind::Board => "▣",
        TreeKind::List => "≡",
        TreeKind::Card => "·",
    };
    let accent = match row.kind {
        TreeKind::Project => Color::Cyan,
        TreeKind::Board => Color::LightGreen,
        TreeKind::List => Color::Yellow,
        TreeKind::Card => Color::White,
    };

    let mut spans = vec![
        Span::raw(indent),
        Span::styled(format!("{branch} "), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{icon} "), Style::default().fg(accent)),
        Span::styled(
            truncate(&row.label, 60),
            Style::default()
                .fg(accent)
                .add_modifier(if matches!(row.kind, TreeKind::Project) {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ];

    if row.live {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "LIVE",
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(meta) = &row.meta {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            truncate(meta, 40),
            Style::default().fg(Color::Gray),
        ));
    }

    ListItem::new(Line::from(spans))
}

#[allow(clippy::too_many_lines)]
fn build_detail_lines(app: &AppState) -> Vec<Line<'static>> {
    let Some(selected) = &app.selected else {
        return vec![Line::from("No selection.")];
    };

    match selected {
        TreeKey::Project(project_id) => {
            if let Some(project) = app
                .projects
                .iter()
                .find(|project| &project.id == project_id)
            {
                let mut lines = vec![
                    detail_title("project"),
                    Line::from(format!("name: {}", project.name)),
                    Line::from(format!("project id: {}", project.id)),
                    Line::from(format!("boards: {}", project.boards.len())),
                    Line::from(""),
                    Line::from("boards:"),
                ];
                lines.extend(project.boards.iter().take(12).map(|board| {
                    let suffix = if board.id == app.subscribed_board_id {
                        "  [live subscribed board]"
                    } else {
                        ""
                    };
                    Line::from(format!("- {} [{}]{}", board.name, board.id, suffix))
                }));
                lines
            } else {
                vec![Line::from("Selected project is no longer available.")]
            }
        }
        TreeKey::Board(board_id) => {
            for project in &app.projects {
                if let Some(board) = project.boards.iter().find(|board| &board.id == board_id) {
                    let mut lines = vec![
                        detail_title("board"),
                        Line::from(format!("name: {}", board.name)),
                        Line::from(format!("board id: {}", board.id)),
                        Line::from(format!("project: {} [{}]", project.name, project.id)),
                        Line::from(format!(
                            "live subscribed: {}",
                            if board.id == app.subscribed_board_id {
                                "yes"
                            } else {
                                "no"
                            }
                        )),
                    ];

                    if board.is_loaded() {
                        lines.push(Line::from(format!(
                            "active lists: {} | cards: {}",
                            board.active_lists.len(),
                            board.total_cards
                        )));
                        lines.push(Line::from(""));
                        lines.push(Line::from("lists:"));
                        lines.extend(board.active_lists.iter().map(|list| {
                            Line::from(format!(
                                "- {} [{}] ({})",
                                list.name, list.id, list.card_count
                            ))
                        }));
                    } else {
                        lines.push(Line::from(""));
                        lines.push(Line::from(
                            "This board is known from the project snapshot, but list/card detail",
                        ));
                        lines.push(Line::from(
                            "has not been loaded yet. The currently subscribed board is loaded",
                        ));
                        lines.push(Line::from("through the live websocket snapshot."));
                    }

                    return lines;
                }
            }

            vec![Line::from("Selected board is no longer available.")]
        }
        TreeKey::List(list_id) => {
            for project in &app.projects {
                for board in &project.boards {
                    if let Some(list) = board.active_lists.iter().find(|list| &list.id == list_id) {
                        let mut lines = vec![
                            detail_title("list"),
                            Line::from(format!("name: {}", list.name)),
                            Line::from(format!("list id: {}", list.id)),
                            Line::from(format!("board: {} [{}]", board.name, board.id)),
                            Line::from(format!("project: {} [{}]", project.name, project.id)),
                            Line::from(format!("cards: {}", list.card_count)),
                            Line::from(""),
                            Line::from("cards:"),
                        ];
                        lines.extend(
                            list.cards
                                .iter()
                                .take(16)
                                .map(|card| Line::from(format!("- {} [{}]", card.name, card.id))),
                        );
                        return lines;
                    }
                }
            }

            vec![Line::from("Selected list is no longer available.")]
        }
        TreeKey::Card(card_id) => {
            for project in &app.projects {
                for board in &project.boards {
                    for list in &board.active_lists {
                        if let Some(card) = list.cards.iter().find(|card| &card.id == card_id) {
                            return vec![
                                detail_title("card"),
                                Line::from(format!("title: {}", card.name)),
                                Line::from(format!("card id: {}", card.id)),
                                Line::from(format!("list: {} [{}]", list.name, list.id)),
                                Line::from(format!("board: {} [{}]", board.name, board.id)),
                                Line::from(format!("project: {} [{}]", project.name, project.id)),
                                Line::from(""),
                                Line::from(
                                    "V1 card editing is planned for title + description only.",
                                ),
                                Line::from(
                                    "Long descriptions will default to $EDITOR rather than inline editing.",
                                ),
                            ];
                        }
                    }
                }
            }

            vec![Line::from("Selected card is no longer available.")]
        }
    }
}

fn detail_title(kind: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("selected {kind}"),
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    )])
}

fn panel_block(title: &str) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn draw_debug_overlay(frame: &mut ratatui::Frame<'_>, area: Rect, app: &AppState) {
    let popup = centered_rect(78, 70, area);
    let debug_items = if app.recent_events.is_empty() {
        vec![ListItem::new("No websocket events yet.")]
    } else {
        app.recent_events
            .iter()
            .map(|record| ListItem::new(format!("{} :: {}", record.name, record.summary)))
            .collect::<Vec<_>>()
    };

    frame.render_widget(Clear, popup);
    frame.render_widget(
        List::new(debug_items).block(panel_block("websocket debug log • press D to close")),
        popup,
    );
}

fn centered_rect(width_percent: u16, height_percent: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1])[1]
}
