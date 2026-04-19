use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;
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
    description: Option<String>,
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
    labels: Vec<LabelItem>,
    card_labels: Vec<CardLabelItem>,
    card_memberships: Vec<CardMembershipItem>,
    users: Vec<UserItem>,
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
    description: Option<String>,
    position: f64,
    is_closed: bool,
    comments_total: usize,
    due_date: Option<String>,
    creator_user_id: Option<String>,
    is_subscribed: bool,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabelItem {
    id: String,
    name: String,
    color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardLabelItem {
    card_id: String,
    label_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardMembershipItem {
    card_id: String,
    user_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserItem {
    id: String,
    name: String,
    username: String,
}

#[derive(Debug, Clone)]
struct LabelSummary {
    id: String,
    name: String,
    color: Option<String>,
}

#[derive(Debug, Clone)]
struct UserSummary {
    id: String,
    name: String,
    username: String,
}

#[derive(Debug, Clone)]
struct CardSummary {
    id: String,
    name: String,
    description: Option<String>,
    position: f64,
    comments_total: usize,
    due_date: Option<String>,
    creator: Option<UserSummary>,
    labels: Vec<LabelSummary>,
    assignees: Vec<UserSummary>,
    is_subscribed: bool,
}

#[derive(Debug, Clone)]
struct ListSummary {
    id: String,
    name: String,
    position: f64,
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
    labels: Vec<LabelSummary>,
    members: Vec<UserSummary>,
}

#[derive(Debug, Clone)]
struct ProjectTree {
    id: String,
    name: String,
    description: Option<String>,
    boards: Vec<BoardSummary>,
}

impl BoardSummary {
    #[allow(clippy::too_many_lines)]
    fn from_snapshot(snapshot: BoardSnapshot) -> Self {
        let BoardSnapshot { item, included } = snapshot;

        let labels_by_id = included
            .labels
            .into_iter()
            .map(|label| {
                let summary = LabelSummary {
                    id: label.id,
                    name: label.name,
                    color: label.color,
                };
                (summary.id.clone(), summary)
            })
            .collect::<HashMap<_, _>>();

        let users_by_id = included
            .users
            .into_iter()
            .map(|user| {
                let summary = UserSummary {
                    id: user.id,
                    name: user.name,
                    username: user.username,
                };
                (summary.id.clone(), summary)
            })
            .collect::<HashMap<_, _>>();

        let mut labels = labels_by_id.values().cloned().collect::<Vec<_>>();
        labels.sort_by(|left, right| left.name.cmp(&right.name));

        let mut members = users_by_id.values().cloned().collect::<Vec<_>>();
        members.sort_by(|left, right| left.name.cmp(&right.name));

        let mut labels_by_card = HashMap::<String, Vec<LabelSummary>>::new();
        for relation in included.card_labels {
            if let Some(label) = labels_by_id.get(&relation.label_id) {
                labels_by_card
                    .entry(relation.card_id)
                    .or_default()
                    .push(label.clone());
            }
        }
        for card_labels in labels_by_card.values_mut() {
            card_labels.sort_by(|left, right| left.name.cmp(&right.name));
        }

        let mut assignees_by_card = HashMap::<String, Vec<UserSummary>>::new();
        for relation in included.card_memberships {
            if let Some(user) = users_by_id.get(&relation.user_id) {
                assignees_by_card
                    .entry(relation.card_id)
                    .or_default()
                    .push(user.clone());
            }
        }
        for assignees in assignees_by_card.values_mut() {
            assignees.sort_by(|left, right| left.name.cmp(&right.name));
        }

        let mut cards = included
            .cards
            .into_iter()
            .filter(|card| !card.is_closed)
            .collect::<Vec<_>>();
        cards.sort_by(|left, right| left.position.total_cmp(&right.position));

        let total_cards = cards.len();

        let mut lists = included.lists;
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
                let position = list.position?;
                if name.trim().is_empty() {
                    return None;
                }

                let list_cards = cards
                    .iter()
                    .filter(|card| card.list_id == list.id)
                    .map(|card| CardSummary {
                        id: card.id.clone(),
                        name: card.name.clone(),
                        description: card.description.clone(),
                        position: card.position,
                        comments_total: card.comments_total,
                        due_date: card.due_date.clone(),
                        creator: card
                            .creator_user_id
                            .as_ref()
                            .and_then(|user_id| users_by_id.get(user_id).cloned()),
                        labels: labels_by_card.get(&card.id).cloned().unwrap_or_default(),
                        assignees: assignees_by_card.get(&card.id).cloned().unwrap_or_default(),
                        is_subscribed: card.is_subscribed,
                    })
                    .collect::<Vec<_>>();

                Some(ListSummary {
                    id: list.id,
                    name,
                    position,
                    card_count: list_cards.len(),
                    cards: list_cards,
                })
            })
            .collect::<Vec<_>>();

        Self {
            id: item.id,
            name: item.name,
            project_id: item.project_id,
            position: item.position,
            total_cards,
            active_lists,
            labels,
            members,
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
            labels: Vec::new(),
            members: Vec::new(),
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
    payload: Option<Value>,
}

#[derive(Debug, Clone)]
enum AppEvent {
    SocketConnecting,
    SocketLive(BoardSummary),
    BoardHydrated(BoardSummary),
    BoardLoadFailed { board_id: String, message: String },
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
    loading_boards: HashSet<String>,
    board_errors: HashMap<String, String>,
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
        let loading_boards = HashSet::new();
        let board_errors = HashMap::new();

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
            loading_boards,
            board_errors,
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
                self.loading_boards.remove(&board.id);
                self.board_errors.remove(&board.id);
                self.merge_board(&board);
                self.board = Some(board);
                self.status = ConnectionState::Live;
            }
            AppEvent::BoardHydrated(board) => {
                self.loading_boards.remove(&board.id);
                self.board_errors.remove(&board.id);
                self.merge_board(&board);
                self.refresh_subscribed_board_cache();
            }
            AppEvent::BoardLoadFailed { board_id, message } => {
                self.loading_boards.remove(&board_id);
                self.board_errors.insert(board_id.clone(), message.clone());
                self.recent_events.insert(
                    0,
                    LiveEventRecord {
                        name: "boardLoadError".to_string(),
                        summary: format!("{board_id} :: {message}"),
                        payload: None,
                    },
                );
                self.recent_events.truncate(24);
            }
            AppEvent::SocketError(message) => {
                self.status = ConnectionState::Error(message);
            }
            AppEvent::LiveEvent(record) => {
                self.apply_live_payload(&record);
                self.recent_events.insert(0, record);
                self.recent_events.truncate(24);
            }
        }
    }

    fn merge_board(&mut self, board: &BoardSummary) {
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
            }
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
                let board_loading = self.loading_boards.contains(&board.id);
                let board_meta = if board.id == self.subscribed_board_id && !board_loaded {
                    Some("live target • waiting websocket snapshot".to_string())
                } else if board_loading {
                    Some("loading board snapshot…".to_string())
                } else if let Some(message) = self.board_errors.get(&board.id) {
                    Some(format!("load failed • {}", truncate(message, 40)))
                } else if board_loaded {
                    Some(format!(
                        "{} lists • {} cards",
                        board.active_lists.len(),
                        board.total_cards
                    ))
                } else {
                    Some("unloaded • press → to hydrate".to_string())
                };

                rows.push(TreeRow {
                    key: TreeKey::Board(board.id.clone()),
                    parent: Some(TreeKey::Project(project.id.clone())),
                    depth: 1,
                    kind: TreeKind::Board,
                    label: board.name.clone(),
                    meta: board_meta,
                    has_children: !board_loaded || !board.active_lists.is_empty(),
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

    fn expand_or_descend(&mut self) -> Option<String> {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return None;
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
                let board_loaded = self
                    .projects
                    .iter()
                    .flat_map(|project| project.boards.iter())
                    .find(|board| board.id == *board_id)
                    .is_some_and(BoardSummary::is_loaded);

                if !board_loaded && board_id != &self.subscribed_board_id {
                    self.expanded_boards.insert(board_id.clone());
                    if self.loading_boards.insert(board_id.clone()) {
                        self.board_errors.remove(board_id);
                        return Some(board_id.clone());
                    }
                } else if row.has_children && !row.expanded {
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

        None
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

    fn apply_live_payload(&mut self, record: &LiveEventRecord) {
        let Some(payload) = record.payload.as_ref() else {
            return;
        };

        match record.name.as_str() {
            "cardUpdate" | "cardCreate" => self.apply_card_upsert(payload),
            "cardDelete" => self.apply_card_delete(payload),
            "listUpdate" | "listCreate" => self.apply_list_upsert(payload),
            "listDelete" => self.apply_list_delete(payload),
            _ => return,
        }

        self.refresh_subscribed_board_cache();
        self.ensure_selected_visible();
    }

    fn apply_card_upsert(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(card_id) = json_string(item, "id").map(ToOwned::to_owned) else {
            return;
        };

        let Some(board) = self.subscribed_board_mut() else {
            return;
        };

        let existing = remove_card_from_board(board, &card_id);
        if json_bool(item, "isClosed").unwrap_or(false) {
            recount_board(board);
            return;
        }

        let Some(list_id) = json_string(item, "listId") else {
            return;
        };

        let creator = match json_string_field(item, "creatorUserId") {
            JsonField::Value(user_id) => board
                .members
                .iter()
                .find(|user| user.id == user_id)
                .cloned(),
            JsonField::Null => None,
            JsonField::Missing => existing.as_ref().and_then(|card| card.creator.clone()),
        };

        let position = json_f64(item, "position")
            .or_else(|| existing.as_ref().map(|card| card.position))
            .unwrap_or(f64::MAX);

        let description = match json_string_field(item, "description") {
            JsonField::Value(value) => Some(value),
            JsonField::Null => None,
            JsonField::Missing => existing.as_ref().and_then(|card| card.description.clone()),
        };

        let due_date = match json_string_field(item, "dueDate") {
            JsonField::Value(value) => Some(value),
            JsonField::Null => None,
            JsonField::Missing => existing.as_ref().and_then(|card| card.due_date.clone()),
        };

        let comments_total = json_u64(item, "commentsTotal")
            .and_then(|value| usize::try_from(value).ok())
            .or_else(|| existing.as_ref().map(|card| card.comments_total))
            .unwrap_or(0);

        let is_subscribed = json_bool(item, "isSubscribed")
            .or_else(|| existing.as_ref().map(|card| card.is_subscribed))
            .unwrap_or(false);

        let labels = existing
            .as_ref()
            .map(|card| card.labels.clone())
            .unwrap_or_default();
        let assignees = existing
            .as_ref()
            .map(|card| card.assignees.clone())
            .unwrap_or_default();

        let Some(list) = board
            .active_lists
            .iter_mut()
            .find(|list| list.id == list_id)
        else {
            recount_board(board);
            return;
        };

        list.cards.push(CardSummary {
            id: card_id,
            name: json_string(item, "name")
                .map(ToOwned::to_owned)
                .or_else(|| existing.as_ref().map(|card| card.name.clone()))
                .unwrap_or_else(|| "untitled".to_string()),
            description,
            position,
            comments_total,
            due_date,
            creator,
            labels,
            assignees,
            is_subscribed,
        });
        list.cards
            .sort_by(|left, right| left.position.total_cmp(&right.position));
        recount_board(board);
    }

    fn apply_card_delete(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(card_id) = json_string(item, "id") else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };
        let _ = remove_card_from_board(board, card_id);
        recount_board(board);
    }

    fn apply_list_upsert(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(list_id) = json_string(item, "id").map(ToOwned::to_owned) else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };

        let position = match json_f64_field(item, "position") {
            JsonField::Value(value) => value,
            JsonField::Null => {
                board.active_lists.retain(|list| list.id != list_id);
                recount_board(board);
                return;
            }
            JsonField::Missing => board
                .active_lists
                .iter()
                .find(|list| list.id == list_id)
                .map_or(f64::MAX, |list| list.position),
        };

        let name = json_string(item, "name")
            .map(ToOwned::to_owned)
            .or_else(|| {
                board
                    .active_lists
                    .iter()
                    .find(|list| list.id == list_id)
                    .map(|list| list.name.clone())
            })
            .unwrap_or_else(|| "Untitled list".to_string());

        if let Some(existing) = board
            .active_lists
            .iter_mut()
            .find(|list| list.id == list_id)
        {
            existing.name = name;
            existing.position = position;
        } else {
            board.active_lists.push(ListSummary {
                id: list_id,
                name,
                position,
                card_count: 0,
                cards: Vec::new(),
            });
        }

        board
            .active_lists
            .sort_by(|left, right| left.position.total_cmp(&right.position));
        recount_board(board);
    }

    fn apply_list_delete(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(list_id) = json_string(item, "id") else {
            return;
        };
        {
            let Some(board) = self.subscribed_board_mut() else {
                return;
            };
            board.active_lists.retain(|list| list.id != list_id);
            recount_board(board);
        }
        self.expanded_lists.remove(list_id);
    }

    fn subscribed_board(&self) -> Option<&BoardSummary> {
        self.projects
            .iter()
            .flat_map(|project| project.boards.iter())
            .find(|board| board.id == self.subscribed_board_id)
    }

    fn subscribed_board_mut(&mut self) -> Option<&mut BoardSummary> {
        let board_id = self.subscribed_board_id.clone();
        self.projects
            .iter_mut()
            .find_map(|project| project.boards.iter_mut().find(|board| board.id == board_id))
    }

    fn refresh_subscribed_board_cache(&mut self) {
        self.board = self.subscribed_board().cloned();
    }

    fn ensure_selected_visible(&mut self) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            self.selected = None;
            return;
        }

        let selected_visible = self
            .selected
            .as_ref()
            .is_some_and(|selected| rows.iter().any(|row| &row.key == selected));

        if !selected_visible {
            self.selected = Some(rows[0].key.clone());
        }
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
    spawn_socket_listener(
        server.clone(),
        token.clone(),
        args.board.clone(),
        tx.clone(),
    );

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
    let runtime = Arc::new(tokio::runtime::Handle::current());

    let result = run_app(&mut terminal, &mut app, &rx, &tx, &server, &token, &runtime);
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
            description: snapshot.item.description,
            boards,
        });
    }

    Ok(trees)
}

async fn fetch_board_summary(
    server: &Url,
    token: &str,
    board_id: &str,
) -> Result<BoardSummary, TuiError> {
    let client = reqwest::Client::new();
    let response = client
        .get(server.join(&format!("api/boards/{board_id}"))?)
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?;
    let snapshot = response.json::<BoardSnapshot>().await?;
    Ok(BoardSummary::from_snapshot(snapshot))
}

fn spawn_board_loader(
    runtime: &Arc<tokio::runtime::Handle>,
    server: &Url,
    token: &str,
    board_id: String,
    tx: &Sender<AppEvent>,
) {
    let runtime = Arc::clone(runtime);
    let server = server.clone();
    let token = token.to_string();
    let tx = tx.clone();
    runtime.spawn(async move {
        match fetch_board_summary(&server, &token, &board_id).await {
            Ok(board) => {
                let _ = tx.send(AppEvent::BoardHydrated(board));
            }
            Err(err) => {
                let _ = tx.send(AppEvent::BoardLoadFailed {
                    board_id,
                    message: err.to_string(),
                });
            }
        }
    });
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
                payload: None,
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
                payload: None,
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
            payload: None,
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
            payload: Some(payload),
        }));
        return Ok(());
    }

    let _ = tx.send(AppEvent::LiveEvent(LiveEventRecord {
        name: "socketPacket".to_string(),
        summary: truncate(packet, 160),
        payload: None,
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

enum JsonField<T> {
    Missing,
    Null,
    Value(T),
}

fn event_item(payload: &Value) -> &Value {
    payload.get("item").unwrap_or(payload)
}

fn json_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn json_string_field(value: &Value, key: &str) -> JsonField<String> {
    match value.get(key) {
        None => JsonField::Missing,
        Some(item) if item.is_null() => JsonField::Null,
        Some(item) => item.as_str().map_or(JsonField::Missing, |text| {
            JsonField::Value(text.to_string())
        }),
    }
}

fn json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn json_f64_field(value: &Value, key: &str) -> JsonField<f64> {
    match value.get(key) {
        None => JsonField::Missing,
        Some(item) if item.is_null() => JsonField::Null,
        Some(item) => item.as_f64().map_or(JsonField::Missing, JsonField::Value),
    }
}

fn remove_card_from_board(board: &mut BoardSummary, card_id: &str) -> Option<CardSummary> {
    for list in &mut board.active_lists {
        if let Some(index) = list.cards.iter().position(|card| card.id == card_id) {
            let removed = list.cards.remove(index);
            list.card_count = list.cards.len();
            return Some(removed);
        }
    }

    None
}

fn recount_board(board: &mut BoardSummary) {
    for list in &mut board.active_lists {
        list.card_count = list.cards.len();
        list.cards
            .sort_by(|left, right| left.position.total_cmp(&right.position));
    }
    board.total_cards = board.active_lists.iter().map(|list| list.cards.len()).sum();
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
            Ok(AppEvent::SocketLive(board) | AppEvent::BoardHydrated(board)) => println!(
                "event: board live: {} [{}] cards={} lists={}",
                board.name,
                board.id,
                board.total_cards,
                board.active_lists.len()
            ),
            Ok(AppEvent::BoardLoadFailed { board_id, message }) => {
                println!("event: board load failed: {board_id} :: {message}");
            }
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
    tx: &Sender<AppEvent>,
    server: &Url,
    token: &str,
    runtime: &Arc<tokio::runtime::Handle>,
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
                        if let Some(board_id) = app.expand_or_descend() {
                            spawn_board_loader(runtime, server, token, board_id, tx);
                        }
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
            "j/k move • h/l collapse/expand • → on unloaded board hydrates snapshot • D debug log • q quits",
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

fn build_detail_lines(app: &AppState) -> Vec<Line<'static>> {
    let Some(selected) = &app.selected else {
        return vec![Line::from("No selection.")];
    };

    match selected {
        TreeKey::Project(project_id) => build_project_detail(app, project_id),
        TreeKey::Board(board_id) => build_board_detail(app, board_id),
        TreeKey::List(list_id) => build_list_detail(app, list_id),
        TreeKey::Card(card_id) => build_card_detail(app, card_id),
    }
}

fn build_project_detail(app: &AppState, project_id: &str) -> Vec<Line<'static>> {
    let Some(project) = app.projects.iter().find(|project| project.id == project_id) else {
        return vec![Line::from("Selected project is no longer available.")];
    };

    let loaded_boards = project
        .boards
        .iter()
        .filter(|board| board.is_loaded())
        .count();
    let mut lines = vec![
        detail_title("project"),
        Line::from(format!("name: {}", project.name)),
        Line::from(format!("project id: {}", project.id)),
        Line::from(format!(
            "boards: {} | loaded boards: {}",
            project.boards.len(),
            loaded_boards
        )),
        Line::from(""),
        Line::from("description:"),
    ];

    push_optional_text_block(
        &mut lines,
        project.description.as_deref(),
        "No project description.",
    );
    lines.push(Line::from(""));
    lines.push(Line::from("boards:"));
    lines.extend(project.boards.iter().take(12).map(|board| {
        let suffix = if board.id == app.subscribed_board_id {
            "  [live subscribed board]"
        } else if board.is_loaded() {
            "  [snapshot loaded]"
        } else {
            ""
        };
        Line::from(format!("- {} [{}]{}", board.name, board.id, suffix))
    }));
    lines
}

fn build_board_detail(app: &AppState, board_id: &str) -> Vec<Line<'static>> {
    for project in &app.projects {
        if let Some(board) = project.boards.iter().find(|board| board.id == board_id) {
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
                    "active lists: {} | cards: {} | labels: {} | members: {}",
                    board.active_lists.len(),
                    board.total_cards,
                    board.labels.len(),
                    board.members.len()
                )));
                lines.push(Line::from(""));
                lines.push(Line::from("lists:"));
                lines.extend(board.active_lists.iter().take(10).map(|list| {
                    Line::from(format!(
                        "- {} [{}] ({})",
                        list.name, list.id, list.card_count
                    ))
                }));
                lines.push(Line::from(""));
                lines.push(Line::from("labels:"));
                if board.labels.is_empty() {
                    lines.push(Line::from("- none"));
                } else {
                    lines.extend(board.labels.iter().take(8).map(|label| {
                        let color = label.color.as_deref().unwrap_or("?");
                        Line::from(format!("- {} [{}] color={}", label.name, label.id, color))
                    }));
                }
                lines.push(Line::from(""));
                lines.push(Line::from("members:"));
                lines.extend(
                    board.members.iter().take(8).map(|member| {
                        Line::from(format!("- {} ({})", member.name, member.username))
                    }),
                );
            } else {
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Board known from project snapshot, but richer list/card detail not loaded yet.",
                ));
                lines.push(Line::from(
                    "Press → or Enter to lazy-load this board snapshot over HTTP.",
                ));
                lines.push(Line::from(
                    "Only subscribed board gets continuous websocket live sync right now.",
                ));
            }

            return lines;
        }
    }

    vec![Line::from("Selected board is no longer available.")]
}

fn build_list_detail(app: &AppState, list_id: &str) -> Vec<Line<'static>> {
    for project in &app.projects {
        for board in &project.boards {
            if let Some(list) = board.active_lists.iter().find(|list| list.id == list_id) {
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
                lines.extend(list.cards.iter().take(14).map(|card| {
                    let label_meta = if card.labels.is_empty() {
                        String::new()
                    } else {
                        format!("  [{}]", join_label_names(&card.labels))
                    };
                    Line::from(format!("- {} [{}]{}", card.name, card.id, label_meta))
                }));
                return lines;
            }
        }
    }

    vec![Line::from("Selected list is no longer available.")]
}

fn build_card_detail(app: &AppState, card_id: &str) -> Vec<Line<'static>> {
    for project in &app.projects {
        for board in &project.boards {
            for list in &board.active_lists {
                if let Some(card) = list.cards.iter().find(|card| card.id == card_id) {
                    let mut lines = vec![
                        detail_title("card"),
                        Line::from(format!("title: {}", card.name)),
                        Line::from(format!("card id: {}", card.id)),
                        Line::from(format!("list: {} [{}]", list.name, list.id)),
                        Line::from(format!("board: {} [{}]", board.name, board.id)),
                        Line::from(format!("project: {} [{}]", project.name, project.id)),
                        Line::from(format!(
                            "due: {} | comments: {} | subscribed: {}",
                            card.due_date.as_deref().unwrap_or("none"),
                            card.comments_total,
                            if card.is_subscribed { "yes" } else { "no" }
                        )),
                        Line::from(format!(
                            "creator: {}",
                            card.creator.as_ref().map_or_else(
                                || "unknown".to_string(),
                                |creator| format!("{} ({})", creator.name, creator.username),
                            )
                        )),
                        Line::from(format!(
                            "labels: {}",
                            if card.labels.is_empty() {
                                "none".to_string()
                            } else {
                                join_label_names(&card.labels)
                            }
                        )),
                        Line::from(format!(
                            "assignees: {}",
                            if card.assignees.is_empty() {
                                "none".to_string()
                            } else {
                                join_user_names(&card.assignees)
                            }
                        )),
                        Line::from(""),
                        Line::from("description:"),
                    ];
                    push_optional_text_block(
                        &mut lines,
                        card.description.as_deref(),
                        "No card description.",
                    );
                    lines.push(Line::from(""));
                    lines.push(Line::from(
                        "V1 card editing target: title + description, with long-form edits via $EDITOR.",
                    ));
                    return lines;
                }
            }
        }
    }

    vec![Line::from("Selected card is no longer available.")]
}

fn join_label_names(labels: &[LabelSummary]) -> String {
    labels
        .iter()
        .map(|label| label.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_user_names(users: &[UserSummary]) -> String {
    users
        .iter()
        .map(|user| format!("{} ({})", user.name, user.username))
        .collect::<Vec<_>>()
        .join(", ")
}

fn push_optional_text_block(lines: &mut Vec<Line<'static>>, text: Option<&str>, empty: &str) {
    let Some(text) = text else {
        lines.push(Line::from(empty.to_string()));
        return;
    };

    let trimmed = text.trim();
    if trimmed.is_empty() {
        lines.push(Line::from(empty.to_string()));
        return;
    }

    for line in trimmed.lines() {
        lines.push(Line::from(line.to_string()));
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
