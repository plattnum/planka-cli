use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Parser;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyEventKind, KeyModifiers};
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
use tokio::sync::watch;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use url::Url;

const SOCKET_IO_PROTOCOL_VERSION: &str = "4";
const SAILS_IO_SDK_VERSION: &str = "1.2.1";
const SUBSCRIBE_ACK_ID: u64 = 1;
const MIN_SAVE_FEEDBACK_DURATION: Duration = Duration::from_millis(900);

const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut out = Vec::with_capacity(input.len().div_ceil(3) * 4);
    let mut chunks = input.chunks_exact(3);
    for chunk in chunks.by_ref() {
        let b0 = chunk[0];
        let b1 = chunk[1];
        let b2 = chunk[2];
        out.push(BASE64_TABLE[((b0 >> 2) & 0x3F) as usize]);
        out.push(BASE64_TABLE[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize]);
        out.push(BASE64_TABLE[(((b1 << 2) | (b2 >> 6)) & 0x3F) as usize]);
        out.push(BASE64_TABLE[(b2 & 0x3F) as usize]);
    }
    let rem = chunks.remainder();
    match rem.len() {
        0 => {}
        1 => {
            let b0 = rem[0];
            out.push(BASE64_TABLE[((b0 >> 2) & 0x3F) as usize]);
            out.push(BASE64_TABLE[((b0 << 4) & 0x3F) as usize]);
            out.push(b'=');
            out.push(b'=');
        }
        2 => {
            let b0 = rem[0];
            let b1 = rem[1];
            out.push(BASE64_TABLE[((b0 >> 2) & 0x3F) as usize]);
            out.push(BASE64_TABLE[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize]);
            out.push(BASE64_TABLE[((b1 << 2) & 0x3F) as usize]);
            out.push(b'=');
        }
        _ => unreachable!(),
    }
    String::from_utf8(out).expect("base64 alphabet is ASCII")
}

fn write_osc52_clipboard(text: &str) -> io::Result<()> {
    use std::io::Write;
    let encoded = base64_encode(text.as_bytes());
    let mut out = io::stdout().lock();
    out.write_all(b"\x1b]52;c;")?;
    out.write_all(encoded.as_bytes())?;
    out.write_all(b"\x07")?;
    out.flush()
}

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

    /// Board ID to subscribe to for live updates at startup. Optional —
    /// when omitted, the TUI launches into the projects view with no
    /// live target, and the user can promote any board to live later
    /// with the `L` keybinding.
    #[arg(long, env = "PLNK_TUI_BOARD")]
    board: Option<String>,

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
    attachments: Vec<AttachmentItem>,
    task_lists: Vec<TaskListItem>,
    tasks: Vec<TaskItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoardListItem {
    id: String,
    name: Option<String>,
    position: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
struct CommentIncluded {
    users: Vec<UserItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentItem {
    id: String,
    text: String,
    created_at: String,
    updated_at: Option<String>,
    user_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CommentsResponse {
    items: Vec<CommentItem>,
    included: CommentIncluded,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentItem {
    id: String,
    card_id: String,
    name: String,
    data: AttachmentData,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentData {
    url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskListItem {
    id: String,
    card_id: String,
    name: String,
    position: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskItem {
    id: String,
    task_list_id: String,
    name: String,
    is_completed: bool,
    assignee_user_id: Option<String>,
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
struct AttachmentSummary {
    id: String,
    name: String,
    url: Option<String>,
}

#[derive(Debug, Clone)]
struct TaskSummary {
    id: String,
    name: String,
    is_completed: bool,
    assignee: Option<UserSummary>,
}

#[derive(Debug, Clone)]
struct TaskListSummary {
    id: String,
    name: String,
    tasks: Vec<TaskSummary>,
}

#[derive(Debug, Clone)]
struct CommentSummary {
    id: String,
    text: String,
    created_at: String,
    updated_at: Option<String>,
    author: Option<UserSummary>,
}

#[derive(Debug, Clone)]
struct CardDraft {
    card_id: String,
    base_title: String,
    draft_title: String,
    base_description: Option<String>,
    draft_description: Option<String>,
    remote_changed: bool,
}

impl CardDraft {
    fn is_dirty(&self) -> bool {
        self.base_title != self.draft_title || self.base_description != self.draft_description
    }
}

#[derive(Debug, Clone)]
struct InlineEditorState {
    card_id: String,
    buffer: String,
    cursor: usize,
}

#[derive(Debug, Clone)]
struct FilterEditorState {
    buffer: String,
    cursor: usize,
}

#[derive(Debug, Clone)]
enum DraftFieldUpdate<T> {
    Unchanged,
    Set(T),
    Clear,
}

#[derive(Debug, Clone)]
enum SaveCompletion {
    Succeeded { card_id: String, item: Value },
    Failed { card_id: String, message: String },
}

#[derive(Debug, Clone)]
struct PendingSaveCompletion {
    ready_at: Instant,
    completion: SaveCompletion,
}

#[derive(Debug, Clone)]
struct CardSummary {
    id: String,
    name: String,
    description: Option<String>,
    position: f64,
    is_closed: bool,
    comments_total: usize,
    due_date: Option<String>,
    creator: Option<UserSummary>,
    labels: Vec<LabelSummary>,
    assignees: Vec<UserSummary>,
    attachments: Vec<AttachmentSummary>,
    task_lists: Vec<TaskListSummary>,
    is_subscribed: bool,
}

#[derive(Debug, Clone)]
struct ListSummary {
    id: String,
    name: String,
    position: f64,
    card_count: usize,
    active_card_count: usize,
    closed_card_count: usize,
    cards: Vec<CardSummary>,
}

#[derive(Debug, Clone)]
struct BoardSummary {
    id: String,
    name: String,
    project_id: String,
    position: f64,
    total_cards: usize,
    active_card_count: usize,
    closed_card_count: usize,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct FastCopy {
    breadcrumb: String,
    json: String,
    command: String,
}

fn fast_copy_for(projects: &[ProjectTree], selected: &TreeKey) -> Option<FastCopy> {
    let target_card_id = match selected {
        TreeKey::Card(id) | TreeKey::GroupedCard { card_id: id, .. } => Some(id.as_str()),
        _ => None,
    };
    let target_list_id = match selected {
        TreeKey::List(id) | TreeKey::LabelGroup { list_id: id, .. } => Some(id.as_str()),
        _ => None,
    };
    let target_board_id = match selected {
        TreeKey::Board(id) => Some(id.as_str()),
        _ => None,
    };
    let target_project_id = match selected {
        TreeKey::Project(id) => Some(id.as_str()),
        _ => None,
    };

    for project in projects {
        if Some(project.id.as_str()) == target_project_id {
            return Some(build_payload(project, None, None, None));
        }
        for board in &project.boards {
            if Some(board.id.as_str()) == target_board_id {
                return Some(build_payload(project, Some(board), None, None));
            }
            for list in &board.active_lists {
                if Some(list.id.as_str()) == target_list_id {
                    return Some(build_payload(project, Some(board), Some(list), None));
                }
                if let Some(card_id) = target_card_id {
                    if let Some(card) = list.cards.iter().find(|card| card.id == card_id) {
                        return Some(build_payload(project, Some(board), Some(list), Some(card)));
                    }
                }
            }
        }
    }
    None
}

fn build_payload(
    project: &ProjectTree,
    board: Option<&BoardSummary>,
    list: Option<&ListSummary>,
    card: Option<&CardSummary>,
) -> FastCopy {
    let mut breadcrumb_parts = vec![project.name.as_str()];
    let mut json_text = format!(
        r#"{{"project":{}"#,
        id_name_object(&project.id, &project.name)
    );

    if let Some(board) = board {
        breadcrumb_parts.push(&board.name);
        json_text.push_str(r#","board":"#);
        json_text.push_str(&id_name_object(&board.id, &board.name));
    }
    if let Some(list) = list {
        breadcrumb_parts.push(&list.name);
        json_text.push_str(r#","list":"#);
        json_text.push_str(&id_name_object(&list.id, &list.name));
    }
    if let Some(card) = card {
        breadcrumb_parts.push(&card.name);
        json_text.push_str(r#","card":"#);
        json_text.push_str(&id_name_object(&card.id, &card.name));
    }
    json_text.push('}');

    let breadcrumb = breadcrumb_parts.join(" > ");

    let snapshot_cmd = match (board, list, card) {
        (_, _, Some(card)) => format!("plnk card snapshot {} --output json", card.id),
        (_, Some(list), None) => format!("plnk list get {} --output json", list.id),
        (Some(board), None, None) => format!("plnk board snapshot {} --output json", board.id),
        (None, None, None) => format!("plnk project snapshot {} --output json", project.id),
    };
    // Names are user-controlled on the Planka server. A newline in a name would
    // break out of the `#` comment line and put attacker-controlled text on its
    // own line, which the user's shell would execute on paste. Strip control
    // characters before embedding the breadcrumb in shell-pasted output.
    let safe_breadcrumb = sanitize_shell_comment(&breadcrumb);
    let command = format!("# {safe_breadcrumb}\n{snapshot_cmd}\n");

    FastCopy {
        breadcrumb,
        json: json_text,
        command,
    }
}

fn id_name_object(id: &str, name: &str) -> String {
    serde_json::to_string(&json!({ "id": id, "name": name }))
        .expect("id/name object is JSON-serializable")
}

fn sanitize_shell_comment(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
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

        let mut attachments_by_card = HashMap::<String, Vec<AttachmentSummary>>::new();
        for attachment in included.attachments {
            attachments_by_card
                .entry(attachment.card_id)
                .or_default()
                .push(AttachmentSummary {
                    id: attachment.id,
                    name: attachment.name,
                    url: attachment.data.url,
                });
        }
        for attachments in attachments_by_card.values_mut() {
            attachments.sort_by(|left, right| left.name.cmp(&right.name));
        }

        let mut tasks_by_task_list = HashMap::<String, Vec<TaskSummary>>::new();
        for task in included.tasks {
            tasks_by_task_list
                .entry(task.task_list_id)
                .or_default()
                .push(TaskSummary {
                    id: task.id,
                    name: task.name,
                    is_completed: task.is_completed,
                    assignee: task
                        .assignee_user_id
                        .as_ref()
                        .and_then(|user_id| users_by_id.get(user_id).cloned()),
                });
        }
        for tasks in tasks_by_task_list.values_mut() {
            tasks.sort_by(|left, right| left.name.cmp(&right.name));
        }

        let mut task_lists_by_card = HashMap::<String, Vec<TaskListSummary>>::new();
        let mut task_lists = included.task_lists;
        task_lists.sort_by(|left, right| left.position.total_cmp(&right.position));
        for task_list in task_lists {
            task_lists_by_card
                .entry(task_list.card_id)
                .or_default()
                .push(TaskListSummary {
                    id: task_list.id.clone(),
                    name: task_list.name,
                    tasks: tasks_by_task_list.remove(&task_list.id).unwrap_or_default(),
                });
        }
        for task_lists in task_lists_by_card.values_mut() {
            task_lists.sort_by(|left, right| left.name.cmp(&right.name));
        }

        let mut cards = included.cards;
        cards.sort_by(|left, right| left.position.total_cmp(&right.position));

        let total_cards = cards.len();
        let active_card_count = cards.iter().filter(|card| !card.is_closed).count();
        let closed_card_count = total_cards.saturating_sub(active_card_count);

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
                        is_closed: card.is_closed,
                        comments_total: card.comments_total,
                        due_date: card.due_date.clone(),
                        creator: card
                            .creator_user_id
                            .as_ref()
                            .and_then(|user_id| users_by_id.get(user_id).cloned()),
                        labels: labels_by_card.get(&card.id).cloned().unwrap_or_default(),
                        assignees: assignees_by_card.get(&card.id).cloned().unwrap_or_default(),
                        attachments: attachments_by_card
                            .get(&card.id)
                            .cloned()
                            .unwrap_or_default(),
                        task_lists: task_lists_by_card
                            .get(&card.id)
                            .cloned()
                            .unwrap_or_default(),
                        is_subscribed: card.is_subscribed,
                    })
                    .collect::<Vec<_>>();
                let active_count = list_cards.iter().filter(|card| !card.is_closed).count();
                let closed_count = list_cards.len().saturating_sub(active_count);

                Some(ListSummary {
                    id: list.id,
                    name,
                    position,
                    card_count: list_cards.len(),
                    active_card_count: active_count,
                    closed_card_count: closed_count,
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
            active_card_count,
            closed_card_count,
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
            active_card_count: 0,
            closed_card_count: 0,
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
    SocketConnecting {
        session_id: u64,
        board_id: String,
    },
    SocketLive {
        session_id: u64,
        board: BoardSummary,
    },
    ProjectRefreshed {
        project: ProjectTree,
        board_errors: Vec<(String, String)>,
    },
    ProjectRefreshFailed {
        project_id: String,
        message: String,
    },
    BoardHydrated(BoardSummary),
    BoardLoadFailed {
        board_id: String,
        message: String,
    },
    CardCommentsLoaded {
        card_id: String,
        comments: Vec<CommentSummary>,
    },
    CardCommentsLoadFailed {
        card_id: String,
        message: String,
    },
    CardSaveSucceeded {
        card_id: String,
        item: Value,
    },
    CardSaveFailed {
        card_id: String,
        message: String,
    },
    SocketError {
        session_id: u64,
        message: String,
    },
    LiveEvent {
        session_id: u64,
        record: LiveEventRecord,
    },
}

#[derive(Debug, Clone)]
enum ConnectionState {
    /// No board has been promoted to the live target yet — the TUI is in
    /// the projects-first explorer state and no websocket is running.
    Idle,
    Loading,
    Connecting,
    Live,
    Error(String),
}

impl ConnectionState {
    fn label(&self) -> String {
        match self {
            Self::Idle => "no live target".to_string(),
            Self::Loading => "loading".to_string(),
            Self::Connecting => "connecting raw websocket".to_string(),
            Self::Live => "live websocket connected".to_string(),
            Self::Error(message) => format!("error: {message}"),
        }
    }

    fn style(&self) -> Style {
        match self {
            Self::Idle => Style::default().fg(Color::DarkGray),
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
    LabelGroup {
        board_id: String,
        list_id: String,
        label_id: Option<String>,
    },
    Card(String),
    GroupedCard {
        group_key: String,
        card_id: String,
    },
}

#[derive(Debug, Clone, Copy)]
enum TreeKind {
    Project,
    Board,
    List,
    LabelGroup,
    Card,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExplorerView {
    Hierarchy,
    Labels,
}

impl ExplorerView {
    fn label(self) -> &'static str {
        match self {
            Self::Hierarchy => "hierarchy",
            Self::Labels => "labels",
        }
    }

    fn toggle(self) -> Self {
        match self {
            Self::Hierarchy => Self::Labels,
            Self::Labels => Self::Hierarchy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneFocus {
    Explorer,
    Details,
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
struct LabelGroupCard {
    card_id: String,
    card_name: String,
    list_id: String,
    list_name: String,
    card_position: f64,
    is_closed: bool,
}

#[derive(Debug, Clone)]
struct LabelGroupSummary {
    board_id: String,
    list_id: String,
    label_id: Option<String>,
    label_name: String,
    color: Option<String>,
    active_card_count: usize,
    closed_card_count: usize,
    cards: Vec<LabelGroupCard>,
}

#[derive(Debug, Clone)]
struct AppState {
    server: String,
    login: String,
    current_user: CurrentUser,
    projects: Vec<ProjectTree>,
    board: Option<BoardSummary>,
    /// Board the websocket is currently subscribed to. `None` when no
    /// live target has been promoted yet (TUI-012).
    subscribed_board_id: Option<String>,
    active_socket_session_id: u64,
    status: ConnectionState,
    recent_events: Vec<LiveEventRecord>,
    expanded_projects: HashSet<String>,
    expanded_boards: HashSet<String>,
    expanded_lists: HashSet<String>,
    expanded_label_groups: HashSet<String>,
    loading_boards: HashSet<String>,
    loading_projects: HashSet<String>,
    board_errors: HashMap<String, String>,
    selected: Option<TreeKey>,
    explorer_view: ExplorerView,
    filter_query: String,
    filter_editor: Option<FilterEditorState>,
    focus: PaneFocus,
    detail_scroll: usize,
    card_comments: HashMap<String, Vec<CommentSummary>>,
    loading_card_comments: HashSet<String>,
    card_comment_errors: HashMap<String, String>,
    card_draft: Option<CardDraft>,
    title_editor: Option<InlineEditorState>,
    saving_card: Option<String>,
    save_started_at: Option<Instant>,
    pending_save_completion: Option<PendingSaveCompletion>,
    notice: Option<String>,
    show_debug_log: bool,
}

#[derive(Debug, Default)]
struct SocketSessionState {
    engine_sid: Option<String>,
    namespace_connected: bool,
    subscribe_sent: bool,
}

#[derive(Debug, Clone)]
enum HierarchyRefreshTarget {
    Project {
        project_id: String,
        loaded_board_ids: HashSet<String>,
    },
    Board {
        board_id: String,
        comment_card_id: Option<String>,
    },
}

impl AppState {
    fn new(
        server: String,
        login: String,
        current_user: CurrentUser,
        projects: Vec<ProjectTree>,
        subscribed_board_id: Option<String>,
        active_socket_session_id: u64,
    ) -> Self {
        let mut expanded_projects = HashSet::new();
        let mut expanded_boards = HashSet::new();
        let expanded_lists = HashSet::new();
        let expanded_label_groups = HashSet::new();
        let loading_boards = HashSet::new();
        let loading_projects = HashSet::new();
        let board_errors = HashMap::new();
        let card_comments = HashMap::new();
        let loading_card_comments = HashSet::new();
        let card_comment_errors = HashMap::new();
        let card_draft = None;
        let title_editor = None;
        let saving_card = None;
        let save_started_at = None;
        let pending_save_completion = None;
        let notice = None;
        let filter_query = String::new();
        let filter_editor = None;

        let selected = subscribed_board_id
            .as_deref()
            .and_then(|target_id| {
                projects
                    .iter()
                    .find(|project| project.boards.iter().any(|board| board.id == target_id))
                    .map(|project| {
                        expanded_projects.insert(project.id.clone());
                        expanded_boards.insert(target_id.to_string());
                        TreeKey::Board(target_id.to_string())
                    })
            })
            .or_else(|| {
                projects
                    .first()
                    .map(|project| TreeKey::Project(project.id.clone()))
            });

        let initial_status = if subscribed_board_id.is_some() {
            ConnectionState::Loading
        } else {
            ConnectionState::Idle
        };

        Self {
            server,
            login,
            current_user,
            projects,
            board: None,
            subscribed_board_id,
            active_socket_session_id,
            status: initial_status,
            recent_events: Vec::new(),
            expanded_projects,
            expanded_boards,
            expanded_lists,
            expanded_label_groups,
            loading_boards,
            loading_projects,
            board_errors,
            selected,
            explorer_view: ExplorerView::Hierarchy,
            filter_query,
            filter_editor,
            focus: PaneFocus::Explorer,
            detail_scroll: 0,
            card_comments,
            loading_card_comments,
            card_comment_errors,
            card_draft,
            title_editor,
            saving_card,
            save_started_at,
            pending_save_completion,
            notice,
            show_debug_log: false,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn apply(&mut self, event: AppEvent) {
        match event {
            AppEvent::SocketConnecting {
                session_id,
                board_id,
            } => {
                if session_id != self.active_socket_session_id
                    || Some(&board_id) != self.subscribed_board_id.as_ref()
                {
                    return;
                }
                self.status = ConnectionState::Connecting;
            }
            AppEvent::SocketLive { session_id, board } => {
                if session_id != self.active_socket_session_id
                    || Some(&board.id) != self.subscribed_board_id.as_ref()
                {
                    return;
                }
                self.loading_boards.remove(&board.id);
                self.board_errors.remove(&board.id);
                self.merge_board(&board);
                self.board = Some(board);
                self.status = ConnectionState::Live;
            }
            AppEvent::ProjectRefreshed {
                project,
                board_errors,
            } => {
                self.loading_projects.remove(&project.id);
                for board in &project.boards {
                    self.loading_boards.remove(&board.id);
                    self.board_errors.remove(&board.id);
                }
                for (board_id, message) in board_errors {
                    self.board_errors.insert(board_id, message);
                }
                self.merge_project(project);
                self.refresh_subscribed_board_cache();
            }
            AppEvent::ProjectRefreshFailed {
                project_id,
                message,
            } => {
                self.loading_projects.remove(&project_id);
                self.recent_events.insert(
                    0,
                    LiveEventRecord {
                        name: "projectRefreshError".to_string(),
                        summary: format!("{project_id} :: {message}"),
                        payload: None,
                    },
                );
                self.recent_events.truncate(24);
                self.set_notice(format!(
                    "Project refresh failed: {}",
                    truncate(&message, 80)
                ));
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
                self.set_notice(format!("Board refresh failed: {}", truncate(&message, 80)));
            }
            AppEvent::CardCommentsLoaded { card_id, comments } => {
                self.loading_card_comments.remove(&card_id);
                self.card_comment_errors.remove(&card_id);
                self.card_comments.insert(card_id, comments);
            }
            AppEvent::CardCommentsLoadFailed { card_id, message } => {
                self.loading_card_comments.remove(&card_id);
                self.card_comment_errors.insert(card_id, message.clone());
            }
            AppEvent::CardSaveSucceeded { card_id, item } => {
                self.finish_or_defer_save(SaveCompletion::Succeeded { card_id, item });
            }
            AppEvent::CardSaveFailed { card_id, message } => {
                self.finish_or_defer_save(SaveCompletion::Failed { card_id, message });
            }
            AppEvent::SocketError {
                session_id,
                message,
            } => {
                if session_id != self.active_socket_session_id {
                    return;
                }
                self.status = ConnectionState::Error(message);
            }
            AppEvent::LiveEvent { session_id, record } => {
                if session_id != self.active_socket_session_id {
                    return;
                }
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

            if Some(&board.id) == self.subscribed_board_id.as_ref() {
                self.expanded_projects.insert(project.id.clone());
                self.expanded_boards.insert(board.id.clone());
            }
        }
    }

    fn merge_project(&mut self, project: ProjectTree) {
        if let Some(existing) = self.projects.iter_mut().find(|item| item.id == project.id) {
            *existing = project;
        } else {
            self.projects.push(project);
        }
    }

    fn active_filter_query(&self) -> &str {
        self.filter_editor
            .as_ref()
            .map_or(self.filter_query.as_str(), |editor| editor.buffer.as_str())
    }

    fn has_active_filter(&self) -> bool {
        !self.active_filter_query().trim().is_empty()
    }

    #[allow(clippy::too_many_lines)]
    fn all_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();

        for project in &self.projects {
            rows.push(TreeRow {
                key: TreeKey::Project(project.id.clone()),
                parent: None,
                depth: 0,
                kind: TreeKind::Project,
                label: project.name.clone(),
                meta: if self.loading_projects.contains(&project.id) {
                    Some("refreshing hierarchy…".to_string())
                } else {
                    Some(format!("{} boards", project.boards.len()))
                },
                has_children: !project.boards.is_empty(),
                expanded: self.expanded_projects.contains(&project.id),
                live: false,
            });

            for board in &project.boards {
                let board_loaded = board.is_loaded();
                let board_loading = self.loading_boards.contains(&board.id);
                let board_meta = if self.is_live_target(&board.id)
                    && !matches!(self.status, ConnectionState::Live)
                {
                    Some("switching live target • waiting websocket snapshot".to_string())
                } else if board_loading {
                    Some("loading board snapshot…".to_string())
                } else if let Some(message) = self.board_errors.get(&board.id) {
                    Some(format!("load failed • {}", truncate(message, 40)))
                } else if board_loaded {
                    Some(format!(
                        "{} lists • {} active • {} closed",
                        board.active_lists.len(),
                        board.active_card_count,
                        board.closed_card_count
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
                    expanded: self.expanded_boards.contains(&board.id),
                    live: self.is_live_connected(&board.id),
                });

                if !board_loaded {
                    continue;
                }

                match self.explorer_view {
                    ExplorerView::Hierarchy => {
                        for list in &board.active_lists {
                            rows.push(TreeRow {
                                key: TreeKey::List(list.id.clone()),
                                parent: Some(TreeKey::Board(board.id.clone())),
                                depth: 2,
                                kind: TreeKind::List,
                                label: list.name.clone(),
                                meta: Some(format!(
                                    "{} active • {} closed",
                                    list.active_card_count, list.closed_card_count
                                )),
                                has_children: !list.cards.is_empty(),
                                expanded: self.expanded_lists.contains(&list.id),
                                live: false,
                            });

                            for card in &list.cards {
                                rows.push(TreeRow {
                                    key: TreeKey::Card(card.id.clone()),
                                    parent: Some(TreeKey::List(list.id.clone())),
                                    depth: 3,
                                    kind: TreeKind::Card,
                                    label: card.name.clone(),
                                    meta: if card.is_closed {
                                        Some("closed".to_string())
                                    } else {
                                        None
                                    },
                                    has_children: false,
                                    expanded: false,
                                    live: false,
                                });
                            }
                        }
                    }
                    ExplorerView::Labels => {
                        for list in &board.active_lists {
                            let label_groups = label_groups_for_list(board, list);
                            rows.push(TreeRow {
                                key: TreeKey::List(list.id.clone()),
                                parent: Some(TreeKey::Board(board.id.clone())),
                                depth: 2,
                                kind: TreeKind::List,
                                label: list.name.clone(),
                                meta: Some(format!(
                                    "{} groups • {} active • {} closed",
                                    label_groups.len(),
                                    list.active_card_count,
                                    list.closed_card_count
                                )),
                                has_children: !label_groups.is_empty(),
                                expanded: self.expanded_lists.contains(&list.id),
                                live: false,
                            });

                            for group in label_groups {
                                let group_key = label_group_key(
                                    &group.board_id,
                                    &group.list_id,
                                    group.label_id.as_deref(),
                                );
                                rows.push(TreeRow {
                                    key: TreeKey::LabelGroup {
                                        board_id: group.board_id.clone(),
                                        list_id: group.list_id.clone(),
                                        label_id: group.label_id.clone(),
                                    },
                                    parent: Some(TreeKey::List(list.id.clone())),
                                    depth: 3,
                                    kind: TreeKind::LabelGroup,
                                    label: group.label_name.clone(),
                                    meta: Some(format!(
                                        "{} active • {} closed",
                                        group.active_card_count, group.closed_card_count
                                    )),
                                    has_children: !group.cards.is_empty(),
                                    expanded: self.expanded_label_groups.contains(&group_key),
                                    live: false,
                                });

                                for card in group.cards {
                                    let card_meta = if card.is_closed {
                                        "closed".to_string()
                                    } else {
                                        "active".to_string()
                                    };
                                    rows.push(TreeRow {
                                        key: TreeKey::GroupedCard {
                                            group_key: group_key.clone(),
                                            card_id: card.card_id.clone(),
                                        },
                                        parent: Some(TreeKey::LabelGroup {
                                            board_id: group.board_id.clone(),
                                            list_id: group.list_id.clone(),
                                            label_id: group.label_id.clone(),
                                        }),
                                        depth: 4,
                                        kind: TreeKind::Card,
                                        label: card.card_name,
                                        meta: Some(card_meta),
                                        has_children: false,
                                        expanded: false,
                                        live: false,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        rows
    }

    fn apply_expansion_visibility(rows: Vec<TreeRow>) -> Vec<TreeRow> {
        let mut visible = Vec::new();
        let mut expanded_visible = HashSet::new();

        for row in rows {
            let parent_visible = row
                .parent
                .as_ref()
                .is_none_or(|parent| expanded_visible.contains(parent));
            if !parent_visible {
                continue;
            }
            if row.expanded {
                expanded_visible.insert(row.key.clone());
            }
            visible.push(row);
        }

        visible
    }

    fn filter_visible_rows(&self, rows: Vec<TreeRow>) -> Vec<TreeRow> {
        let query = self.active_filter_query().trim();
        if query.is_empty() {
            return Self::apply_expansion_visibility(rows);
        }

        let parent_by_key = rows
            .iter()
            .map(|row| (row.key.clone(), row.parent.clone()))
            .collect::<HashMap<_, _>>();
        let matched_keys = rows
            .iter()
            .filter(|row| filter_matches(query, &row.label))
            .map(|row| row.key.clone())
            .collect::<Vec<_>>();

        if matched_keys.is_empty() {
            return Vec::new();
        }

        let mut included = HashSet::new();
        let mut force_expanded = HashSet::new();
        for key in matched_keys {
            included.insert(key.clone());
            let mut current = parent_by_key.get(&key).cloned().flatten();
            while let Some(parent) = current {
                force_expanded.insert(parent.clone());
                included.insert(parent.clone());
                current = parent_by_key.get(&parent).cloned().flatten();
            }
        }

        rows.into_iter()
            .filter_map(|mut row| {
                if !included.contains(&row.key) {
                    return None;
                }
                row.expanded = force_expanded.contains(&row.key);
                Some(row)
            })
            .collect()
    }

    fn visible_rows(&self) -> Vec<TreeRow> {
        self.filter_visible_rows(self.all_rows())
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

    fn set_selected(&mut self, selected: TreeKey) {
        let changed = self.selected.as_ref() != Some(&selected);
        self.selected = Some(selected);
        if changed {
            self.detail_scroll = 0;
        }
    }

    fn selected_card_id(&self) -> Option<&str> {
        match self.selected.as_ref() {
            Some(TreeKey::Card(card_id) | TreeKey::GroupedCard { card_id, .. }) => {
                Some(card_id.as_str())
            }
            _ => None,
        }
    }

    fn selected_board_id(&self) -> Option<&str> {
        match self.selected.as_ref() {
            Some(TreeKey::Board(board_id)) => Some(board_id.as_str()),
            _ => None,
        }
    }

    fn board_id_for_list(&self, list_id: &str) -> Option<&str> {
        self.projects.iter().find_map(|project| {
            project.boards.iter().find_map(|board| {
                board
                    .active_lists
                    .iter()
                    .any(|list| list.id == list_id)
                    .then_some(board.id.as_str())
            })
        })
    }

    fn board_id_for_card(&self, card_id: &str) -> Option<&str> {
        self.projects.iter().find_map(|project| {
            project.boards.iter().find_map(|board| {
                board
                    .active_lists
                    .iter()
                    .any(|list| list.cards.iter().any(|card| card.id == card_id))
                    .then_some(board.id.as_str())
            })
        })
    }

    fn loaded_board_ids_for_project(&self, project_id: &str) -> HashSet<String> {
        self.projects
            .iter()
            .find(|project| project.id == project_id)
            .map(|project| {
                project
                    .boards
                    .iter()
                    .filter(|board| board.is_loaded() || self.is_live_target(&board.id))
                    .map(|board| board.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn force_card_comments_reload(&mut self, card_id: &str) -> Option<String> {
        self.card_comments.remove(card_id);
        self.card_comment_errors.remove(card_id);
        if self.loading_card_comments.contains(card_id) {
            return None;
        }
        self.loading_card_comments.insert(card_id.to_string());
        Some(card_id.to_string())
    }

    fn queue_hierarchy_refresh(&mut self) -> Option<HierarchyRefreshTarget> {
        let Some(selected) = self.selected.as_ref() else {
            self.set_notice("Select a node to refresh its hierarchy.");
            return None;
        };

        let target = match selected {
            TreeKey::Project(project_id) => HierarchyRefreshTarget::Project {
                project_id: project_id.clone(),
                loaded_board_ids: self.loaded_board_ids_for_project(project_id),
            },
            TreeKey::Board(board_id) => HierarchyRefreshTarget::Board {
                board_id: board_id.clone(),
                comment_card_id: None,
            },
            TreeKey::List(list_id) | TreeKey::LabelGroup { list_id, .. } => {
                HierarchyRefreshTarget::Board {
                    board_id: self.board_id_for_list(list_id)?.to_string(),
                    comment_card_id: None,
                }
            }
            TreeKey::Card(card_id) | TreeKey::GroupedCard { card_id, .. } => {
                HierarchyRefreshTarget::Board {
                    board_id: self.board_id_for_card(card_id)?.to_string(),
                    comment_card_id: Some(card_id.clone()),
                }
            }
        };

        match target {
            HierarchyRefreshTarget::Project {
                project_id,
                loaded_board_ids,
            } => {
                if !self.loading_projects.insert(project_id.clone()) {
                    self.set_notice("Project hierarchy refresh already in progress.");
                    return None;
                }
                self.set_notice("Refreshing project hierarchy…");
                Some(HierarchyRefreshTarget::Project {
                    project_id,
                    loaded_board_ids,
                })
            }
            HierarchyRefreshTarget::Board {
                board_id,
                comment_card_id,
            } => {
                if !self.loading_boards.insert(board_id.clone()) {
                    self.set_notice("Board hierarchy refresh already in progress.");
                    return None;
                }
                self.board_errors.remove(&board_id);
                let comment_card_id = comment_card_id
                    .as_deref()
                    .and_then(|card_id| self.force_card_comments_reload(card_id));
                self.set_notice("Refreshing board hierarchy…");
                Some(HierarchyRefreshTarget::Board {
                    board_id,
                    comment_card_id,
                })
            }
        }
    }

    fn toggle_explorer_view(&mut self) {
        let replacement = match self.selected.as_ref() {
            Some(TreeKey::Card(card_id) | TreeKey::GroupedCard { card_id, .. }) => self
                .card_list_id(card_id)
                .map(|list_id| TreeKey::List(list_id.to_string())),
            Some(TreeKey::List(list_id) | TreeKey::LabelGroup { list_id, .. }) => {
                Some(TreeKey::List(list_id.clone()))
            }
            _ => None,
        };
        if let Some(replacement) = replacement {
            self.set_selected(replacement);
        }
        self.explorer_view = self.explorer_view.toggle();
        self.ensure_selected_visible();
        self.set_notice(format!("Explorer view: {}.", self.explorer_view.label()));
    }

    fn selected_card_summary(&self) -> Option<&CardSummary> {
        let card_id = self.selected_card_id()?;
        self.projects
            .iter()
            .flat_map(|project| project.boards.iter())
            .flat_map(|board| board.active_lists.iter())
            .flat_map(|list| list.cards.iter())
            .find(|card| card.id == card_id)
    }

    fn ensure_card_draft(&mut self) -> Option<&mut CardDraft> {
        let card = self.selected_card_summary()?.clone();
        let card_id = card.id.clone();
        let needs_new = self
            .card_draft
            .as_ref()
            .is_none_or(|draft| draft.card_id != card_id);
        if needs_new {
            self.card_draft = Some(CardDraft {
                card_id,
                base_title: card.name.clone(),
                draft_title: card.name,
                base_description: card.description.clone(),
                draft_description: card.description,
                remote_changed: false,
            });
        }
        self.card_draft.as_mut()
    }

    fn current_card_draft(&self, card_id: &str) -> Option<&CardDraft> {
        self.card_draft
            .as_ref()
            .filter(|draft| draft.card_id == card_id)
    }

    fn board_summary(&self, board_id: &str) -> Option<&BoardSummary> {
        self.projects
            .iter()
            .flat_map(|project| project.boards.iter())
            .find(|board| board.id == board_id)
    }

    fn fast_copy(&self) -> Option<FastCopy> {
        fast_copy_for(&self.projects, self.selected.as_ref()?)
    }

    fn card_list_id(&self, card_id: &str) -> Option<&str> {
        self.projects.iter().find_map(|project| {
            project.boards.iter().find_map(|board| {
                board.active_lists.iter().find_map(|list| {
                    list.cards
                        .iter()
                        .any(|card| card.id == card_id)
                        .then_some(list.id.as_str())
                })
            })
        })
    }

    fn board_project_id(&self, board_id: &str) -> Option<&str> {
        self.projects.iter().find_map(|project| {
            project
                .boards
                .iter()
                .any(|board| board.id == board_id)
                .then_some(project.id.as_str())
        })
    }

    fn is_live_target(&self, board_id: &str) -> bool {
        self.subscribed_board_id
            .as_deref()
            .is_some_and(|id| id == board_id)
    }

    fn is_live_connected(&self, board_id: &str) -> bool {
        self.is_live_target(board_id) && matches!(self.status, ConnectionState::Live)
    }

    fn switch_live_board(&mut self, board_id: &str) {
        let board_name = self
            .board_summary(board_id)
            .map_or_else(|| board_id.to_string(), |board| board.name.clone());
        let was_idle = matches!(self.status, ConnectionState::Idle);
        self.subscribed_board_id = Some(board_id.to_string());
        self.active_socket_session_id = self.active_socket_session_id.saturating_add(1);
        self.status = ConnectionState::Connecting;
        self.board_errors.remove(board_id);
        self.board = self
            .board_summary(board_id)
            .filter(|board| board.is_loaded())
            .cloned();
        if let Some(project_id) = self.board_project_id(board_id) {
            self.expanded_projects.insert(project_id.to_string());
        }
        self.expanded_boards.insert(board_id.to_string());
        let verb = if was_idle { "Promoting" } else { "Switching" };
        self.set_notice(format!("{verb} live target to {board_name}…"));
    }

    fn clear_live_board(&mut self) {
        let board_name = self
            .subscribed_board()
            .map_or_else(|| "current board".to_string(), |board| board.name.clone());
        self.subscribed_board_id = None;
        self.active_socket_session_id = self.active_socket_session_id.saturating_add(1);
        self.status = ConnectionState::Idle;
        self.board = None;
        self.set_notice(format!("Stopped live sync for {board_name}."));
    }

    fn flush_pending_save_completion(&mut self) {
        if self
            .pending_save_completion
            .as_ref()
            .is_some_and(|pending| Instant::now() >= pending.ready_at)
        {
            let Some(pending) = self.pending_save_completion.take() else {
                return;
            };
            self.finish_save(pending.completion);
        }
    }

    fn finish_or_defer_save(&mut self, completion: SaveCompletion) {
        let Some(started_at) = self.save_started_at else {
            self.finish_save(completion);
            return;
        };

        let elapsed = started_at.elapsed();
        if elapsed >= MIN_SAVE_FEEDBACK_DURATION {
            self.finish_save(completion);
            return;
        }

        let Some(remaining) = MIN_SAVE_FEEDBACK_DURATION.checked_sub(elapsed) else {
            self.finish_save(completion);
            return;
        };

        self.pending_save_completion = Some(PendingSaveCompletion {
            ready_at: Instant::now() + remaining,
            completion,
        });
    }

    fn finish_save(&mut self, completion: SaveCompletion) {
        self.saving_card = None;
        self.save_started_at = None;
        self.pending_save_completion = None;
        match completion {
            SaveCompletion::Succeeded { card_id, item } => {
                let payload = serde_json::json!({ "item": item });
                self.apply_card_upsert(&payload);
                if self
                    .card_draft
                    .as_ref()
                    .is_some_and(|draft| draft.card_id == card_id)
                {
                    self.card_draft = None;
                }
                self.notice = Some("Saved card changes.".to_string());
            }
            SaveCompletion::Failed { card_id, message } => {
                self.notice = Some(format!("Save failed for {card_id}: {message}"));
            }
        }
    }

    fn card_summary(&self, card_id: &str) -> Option<&CardSummary> {
        self.projects
            .iter()
            .flat_map(|project| project.boards.iter())
            .flat_map(|board| board.active_lists.iter())
            .flat_map(|list| list.cards.iter())
            .find(|card| card.id == card_id)
    }

    fn saving_card_label(&self) -> Option<&str> {
        self.saving_card.as_deref().and_then(|card_id| {
            self.current_card_draft(card_id)
                .map(|draft| draft.draft_title.as_str())
                .or_else(|| self.card_summary(card_id).map(|card| card.name.as_str()))
        })
    }

    fn has_dirty_card_draft(&self) -> bool {
        self.card_draft.as_ref().is_some_and(CardDraft::is_dirty)
    }

    fn set_notice(&mut self, message: impl Into<String>) {
        self.notice = Some(message.into());
    }

    fn clear_notice(&mut self) {
        self.notice = None;
    }

    fn start_title_editor(&mut self) {
        let Some(draft) = self.ensure_card_draft() else {
            return;
        };
        self.title_editor = Some(InlineEditorState {
            card_id: draft.card_id.clone(),
            buffer: draft.draft_title.clone(),
            cursor: draft.draft_title.chars().count(),
        });
        self.set_notice("Editing card title. Type to edit • Enter apply • Esc cancel.");
    }

    fn cancel_title_editor(&mut self) {
        self.title_editor = None;
        self.set_notice("Cancelled title edit.");
    }

    fn apply_title_editor(&mut self) {
        let Some(editor) = self.title_editor.take() else {
            return;
        };
        if let Some(draft) = self
            .card_draft
            .as_mut()
            .filter(|draft| draft.card_id == editor.card_id)
        {
            draft.draft_title = editor.buffer;
        }
    }

    fn discard_card_draft(&mut self) {
        self.card_draft = None;
        self.title_editor = None;
        self.saving_card = None;
        self.save_started_at = None;
        self.pending_save_completion = None;
        self.clear_notice();
    }

    fn save_request(
        &mut self,
    ) -> Option<(String, DraftFieldUpdate<String>, DraftFieldUpdate<String>)> {
        if self.title_editor.is_some() {
            self.apply_title_editor();
        }
        let draft = self.card_draft.as_ref()?;
        if !draft.is_dirty() {
            self.set_notice("No unsaved card changes.");
            return None;
        }
        if self.saving_card.is_some() {
            self.set_notice("Save already in progress.");
            return None;
        }
        let card_id = draft.card_id.clone();
        let title = if draft.base_title == draft.draft_title {
            DraftFieldUpdate::Unchanged
        } else {
            DraftFieldUpdate::Set(draft.draft_title.clone())
        };
        let description = if draft.base_description == draft.draft_description {
            DraftFieldUpdate::Unchanged
        } else {
            match draft.draft_description.clone() {
                Some(value) => DraftFieldUpdate::Set(value),
                None => DraftFieldUpdate::Clear,
            }
        };
        self.saving_card = Some(card_id.clone());
        self.save_started_at = Some(Instant::now());
        self.pending_save_completion = None;
        self.set_notice("Saving card changes…");
        Some((card_id, title, description))
    }

    fn edit_title_insert(&mut self, ch: char) {
        let Some(editor) = self.title_editor.as_mut() else {
            return;
        };
        let mut chars = editor.buffer.chars().collect::<Vec<_>>();
        chars.insert(editor.cursor, ch);
        editor.cursor = editor.cursor.saturating_add(1);
        editor.buffer = chars.into_iter().collect();
    }

    fn edit_title_backspace(&mut self) {
        let Some(editor) = self.title_editor.as_mut() else {
            return;
        };
        if editor.cursor == 0 {
            return;
        }
        let mut chars = editor.buffer.chars().collect::<Vec<_>>();
        let index = editor.cursor.saturating_sub(1);
        let _ = chars.remove(index);
        editor.cursor = index;
        editor.buffer = chars.into_iter().collect();
    }

    fn edit_title_delete(&mut self) {
        let Some(editor) = self.title_editor.as_mut() else {
            return;
        };
        let mut chars = editor.buffer.chars().collect::<Vec<_>>();
        if editor.cursor >= chars.len() {
            return;
        }
        let _ = chars.remove(editor.cursor);
        editor.buffer = chars.into_iter().collect();
    }

    fn edit_title_move_left(&mut self) {
        if let Some(editor) = self.title_editor.as_mut() {
            editor.cursor = editor.cursor.saturating_sub(1);
        }
    }

    fn edit_title_move_right(&mut self) {
        if let Some(editor) = self.title_editor.as_mut() {
            editor.cursor = editor
                .cursor
                .saturating_add(1)
                .min(editor.buffer.chars().count());
        }
    }

    fn edit_title_move_home(&mut self) {
        if let Some(editor) = self.title_editor.as_mut() {
            editor.cursor = 0;
        }
    }

    fn edit_title_move_end(&mut self) {
        if let Some(editor) = self.title_editor.as_mut() {
            editor.cursor = editor.buffer.chars().count();
        }
    }

    fn start_filter_editor(&mut self) {
        self.focus_explorer();
        self.filter_editor = Some(FilterEditorState {
            buffer: self.filter_query.clone(),
            cursor: self.filter_query.chars().count(),
        });
        self.set_notice("Filter explorer rows by text or glob (*, ?).");
    }

    fn sync_filter_editor(&mut self) {
        let Some(editor) = self.filter_editor.as_ref() else {
            return;
        };
        self.filter_query.clone_from(&editor.buffer);
        self.ensure_selected_visible();
    }

    fn finish_filter_editor(&mut self) {
        self.sync_filter_editor();
        self.filter_editor = None;
        if self.has_active_filter() {
            self.set_notice(format!(
                "Explorer filter active: {}",
                truncate(self.active_filter_query(), 60)
            ));
        } else {
            self.set_notice("Explorer filter cleared.");
        }
    }

    fn clear_or_close_filter_editor(&mut self) {
        let Some(editor) = self.filter_editor.as_mut() else {
            return;
        };
        if editor.buffer.is_empty() {
            self.filter_editor = None;
            self.set_notice("Exited filter mode.");
        } else {
            editor.buffer.clear();
            editor.cursor = 0;
            self.sync_filter_editor();
            self.set_notice("Explorer filter cleared. Esc again to close filter mode.");
        }
    }

    fn edit_filter_insert(&mut self, ch: char) {
        if let Some(editor) = self.filter_editor.as_mut() {
            let mut chars = editor.buffer.chars().collect::<Vec<_>>();
            let cursor = editor.cursor.min(chars.len());
            chars.insert(cursor, ch);
            editor.buffer = chars.into_iter().collect();
            editor.cursor = cursor.saturating_add(1);
        }
        self.sync_filter_editor();
    }

    fn edit_filter_backspace(&mut self) {
        let Some(editor) = self.filter_editor.as_mut() else {
            return;
        };
        if editor.cursor == 0 {
            return;
        }
        let mut chars = editor.buffer.chars().collect::<Vec<_>>();
        let index = editor.cursor.saturating_sub(1);
        let _ = chars.remove(index);
        editor.buffer = chars.into_iter().collect();
        editor.cursor = index;
        self.sync_filter_editor();
    }

    fn edit_filter_delete(&mut self) {
        let Some(editor) = self.filter_editor.as_mut() else {
            return;
        };
        let mut chars = editor.buffer.chars().collect::<Vec<_>>();
        if editor.cursor >= chars.len() {
            return;
        }
        let _ = chars.remove(editor.cursor);
        editor.buffer = chars.into_iter().collect();
        self.sync_filter_editor();
    }

    fn edit_filter_move_left(&mut self) {
        if let Some(editor) = self.filter_editor.as_mut() {
            editor.cursor = editor.cursor.saturating_sub(1);
        }
    }

    fn edit_filter_move_right(&mut self) {
        if let Some(editor) = self.filter_editor.as_mut() {
            editor.cursor = editor
                .cursor
                .saturating_add(1)
                .min(editor.buffer.chars().count());
        }
    }

    fn edit_filter_move_home(&mut self) {
        if let Some(editor) = self.filter_editor.as_mut() {
            editor.cursor = 0;
        }
    }

    fn edit_filter_move_end(&mut self) {
        if let Some(editor) = self.filter_editor.as_mut() {
            editor.cursor = editor.buffer.chars().count();
        }
    }

    fn mark_selected_card_comments_loading(&mut self) -> Option<String> {
        let card_id = self.selected_card_id()?.to_string();
        if self.card_comments.contains_key(&card_id)
            || self.loading_card_comments.contains(&card_id)
            || self.card_comment_errors.contains_key(&card_id)
        {
            return None;
        }
        self.loading_card_comments.insert(card_id.clone());
        Some(card_id)
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            PaneFocus::Explorer => PaneFocus::Details,
            PaneFocus::Details => PaneFocus::Explorer,
        };
    }

    fn focus_explorer(&mut self) {
        self.focus = PaneFocus::Explorer;
    }

    fn focus_details(&mut self) {
        self.focus = PaneFocus::Details;
    }

    fn scroll_details_by(&mut self, delta: i32) {
        let max_scroll = build_detail_lines(self).len().saturating_sub(1);
        let step = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
        if delta.is_negative() {
            self.detail_scroll = self.detail_scroll.saturating_sub(step);
        } else {
            self.detail_scroll = self.detail_scroll.saturating_add(step).min(max_scroll);
        }
    }

    fn scroll_details_to_top(&mut self) {
        self.detail_scroll = 0;
    }

    fn scroll_details_to_bottom(&mut self) {
        self.detail_scroll = build_detail_lines(self).len().saturating_sub(1);
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

        self.set_selected(rows[next].key.clone());
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
                    self.set_selected(next_row.key.clone());
                }
            }
            TreeKey::Board(board_id) => {
                let board_loaded = self
                    .projects
                    .iter()
                    .flat_map(|project| project.boards.iter())
                    .find(|board| board.id == *board_id)
                    .is_some_and(BoardSummary::is_loaded);

                if !board_loaded && Some(board_id) != self.subscribed_board_id.as_ref() {
                    self.expanded_boards.insert(board_id.clone());
                    if self.loading_boards.insert(board_id.clone()) {
                        self.board_errors.remove(board_id);
                        return Some(board_id.clone());
                    }
                } else if row.has_children && !row.expanded {
                    self.expanded_boards.insert(board_id.clone());
                } else if let Some(next_row) = rows.get(index + 1) {
                    self.set_selected(next_row.key.clone());
                }
            }
            TreeKey::List(list_id) => {
                if row.has_children && !row.expanded {
                    self.expanded_lists.insert(list_id.clone());
                } else if let Some(next_row) = rows.get(index + 1) {
                    self.set_selected(next_row.key.clone());
                }
            }
            TreeKey::LabelGroup {
                board_id,
                list_id,
                label_id,
            } => {
                let group_key = label_group_key(board_id, list_id, label_id.as_deref());
                if row.has_children && !row.expanded {
                    self.expanded_label_groups.insert(group_key);
                } else if let Some(next_row) = rows.get(index + 1) {
                    self.set_selected(next_row.key.clone());
                }
            }
            TreeKey::Card(_) | TreeKey::GroupedCard { .. } => {}
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

        if self.has_active_filter() {
            if let Some(parent) = &row.parent {
                self.set_selected(parent.clone());
            }
            return;
        }

        match &row.key {
            TreeKey::Project(project_id) => {
                self.expanded_projects.remove(project_id);
            }
            TreeKey::Board(board_id) => {
                if row.expanded {
                    self.expanded_boards.remove(board_id);
                } else if let Some(parent) = &row.parent {
                    self.set_selected(parent.clone());
                }
            }
            TreeKey::List(list_id) => {
                if row.expanded {
                    self.expanded_lists.remove(list_id);
                } else if let Some(parent) = &row.parent {
                    self.set_selected(parent.clone());
                }
            }
            TreeKey::LabelGroup {
                board_id,
                list_id,
                label_id,
            } => {
                let group_key = label_group_key(board_id, list_id, label_id.as_deref());
                if row.expanded {
                    self.expanded_label_groups.remove(&group_key);
                } else if let Some(parent) = &row.parent {
                    self.set_selected(parent.clone());
                }
            }
            TreeKey::Card(_) | TreeKey::GroupedCard { .. } => {
                if let Some(parent) = &row.parent {
                    self.set_selected(parent.clone());
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

        if self.has_dirty_card_draft()
            && self
                .card_draft
                .as_ref()
                .is_some_and(|draft| payload_touches_card(payload, &draft.card_id))
            && self.saving_card.as_deref() != payload_card_id(payload)
        {
            if let Some(draft) = self.card_draft.as_mut() {
                draft.remote_changed = true;
            }
        }

        match record.name.as_str() {
            "boardUpdate" => self.apply_board_update(payload),
            "cardUpdate" | "cardCreate" => self.apply_card_upsert(payload),
            "cardDelete" => self.apply_card_delete(payload),
            "listUpdate" | "listCreate" => self.apply_list_upsert(payload),
            "listDelete" => self.apply_list_delete(payload),
            "labelCreate" | "labelUpdate" => self.apply_label_upsert(payload),
            "labelDelete" => self.apply_label_delete(payload),
            "cardLabelCreate" => self.apply_card_label_create(payload),
            "cardLabelDelete" => self.apply_card_label_delete(payload),
            "cardMembershipCreate" => self.apply_card_membership_create(payload),
            "cardMembershipDelete" => self.apply_card_membership_delete(payload),
            _ => return,
        }

        self.refresh_subscribed_board_cache();
        self.ensure_selected_visible();
    }

    fn apply_board_update(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(board_id) = json_string(item, "id") else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };
        if board.id != board_id {
            return;
        }

        if let Some(name) = json_string(item, "name") {
            board.name = name.to_string();
        }
    }

    fn apply_card_upsert(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(card_id) = json_string(item, "id").map(ToOwned::to_owned) else {
            return;
        };

        let invalidate_comments = {
            let Some(board) = self.subscribed_board_mut() else {
                return;
            };

            let existing = remove_card_from_board(board, &card_id);

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
            let invalidate_comments = existing
                .as_ref()
                .is_some_and(|card| card.comments_total != comments_total);

            let is_subscribed = json_bool(item, "isSubscribed")
                .or_else(|| existing.as_ref().map(|card| card.is_subscribed))
                .unwrap_or(false);
            let is_closed = json_bool(item, "isClosed")
                .or_else(|| existing.as_ref().map(|card| card.is_closed))
                .unwrap_or(false);

            let labels = existing
                .as_ref()
                .map(|card| card.labels.clone())
                .unwrap_or_default();
            let assignees = existing
                .as_ref()
                .map(|card| card.assignees.clone())
                .unwrap_or_default();
            let attachments = existing
                .as_ref()
                .map(|card| card.attachments.clone())
                .unwrap_or_default();
            let task_lists = existing
                .as_ref()
                .map(|card| card.task_lists.clone())
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
                id: card_id.clone(),
                name: json_string(item, "name")
                    .map(ToOwned::to_owned)
                    .or_else(|| existing.as_ref().map(|card| card.name.clone()))
                    .unwrap_or_else(|| "untitled".to_string()),
                description,
                position,
                is_closed,
                comments_total,
                due_date,
                creator,
                labels,
                assignees,
                attachments,
                task_lists,
                is_subscribed,
            });
            list.cards
                .sort_by(|left, right| left.position.total_cmp(&right.position));
            recount_board(board);
            invalidate_comments
        };

        if invalidate_comments {
            self.card_comments.remove(&card_id);
            self.card_comment_errors.remove(&card_id);
        }
    }

    fn apply_card_delete(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(card_id) = json_string(item, "id") else {
            return;
        };
        {
            let Some(board) = self.subscribed_board_mut() else {
                return;
            };
            let _ = remove_card_from_board(board, card_id);
            recount_board(board);
        }
        self.card_comments.remove(card_id);
        self.card_comment_errors.remove(card_id);
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
                active_card_count: 0,
                closed_card_count: 0,
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

    fn apply_label_upsert(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(label_id) = json_string(item, "id") else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };

        let label_name = json_string(item, "name")
            .map(ToOwned::to_owned)
            .or_else(|| {
                board
                    .labels
                    .iter()
                    .find(|label| label.id == label_id)
                    .map(|label| label.name.clone())
            })
            .unwrap_or_else(|| "Untitled label".to_string());
        let label_color = match json_string_field(item, "color") {
            JsonField::Value(value) => Some(value),
            JsonField::Null => None,
            JsonField::Missing => board
                .labels
                .iter()
                .find(|label| label.id == label_id)
                .and_then(|label| label.color.clone()),
        };

        if let Some(label) = board.labels.iter_mut().find(|label| label.id == label_id) {
            label.name.clone_from(&label_name);
            label.color.clone_from(&label_color);
        } else {
            board.labels.push(LabelSummary {
                id: label_id.to_string(),
                name: label_name.clone(),
                color: label_color.clone(),
            });
        }
        board
            .labels
            .sort_by(|left, right| left.name.cmp(&right.name));

        for list in &mut board.active_lists {
            for card in &mut list.cards {
                if let Some(label) = card.labels.iter_mut().find(|label| label.id == label_id) {
                    label.name.clone_from(&label_name);
                    label.color.clone_from(&label_color);
                    card.labels
                        .sort_by(|left, right| left.name.cmp(&right.name));
                }
            }
        }
    }

    fn apply_label_delete(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(label_id) = json_string(item, "id") else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };
        board.labels.retain(|label| label.id != label_id);
        for list in &mut board.active_lists {
            for card in &mut list.cards {
                card.labels.retain(|label| label.id != label_id);
            }
        }
    }

    fn apply_card_label_create(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(card_id) = json_string(item, "cardId") else {
            return;
        };
        let Some(label_id) = json_string(item, "labelId") else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };
        let Some(label) = board
            .labels
            .iter()
            .find(|label| label.id == label_id)
            .cloned()
        else {
            return;
        };
        let Some(card) = find_card_mut(board, card_id) else {
            return;
        };
        if card.labels.iter().any(|existing| existing.id == label.id) {
            return;
        }
        card.labels.push(label);
        card.labels
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    fn apply_card_label_delete(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(card_id) = json_string(item, "cardId") else {
            return;
        };
        let Some(label_id) = json_string(item, "labelId") else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };
        let Some(card) = find_card_mut(board, card_id) else {
            return;
        };
        card.labels.retain(|label| label.id != label_id);
    }

    fn apply_card_membership_create(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(card_id) = json_string(item, "cardId") else {
            return;
        };
        let Some(user_id) = json_string(item, "userId") else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };
        let Some(user) = board
            .members
            .iter()
            .find(|member| member.id == user_id)
            .cloned()
        else {
            return;
        };
        let Some(card) = find_card_mut(board, card_id) else {
            return;
        };
        if card.assignees.iter().any(|existing| existing.id == user.id) {
            return;
        }
        card.assignees.push(user);
        card.assignees
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    fn apply_card_membership_delete(&mut self, payload: &Value) {
        let item = event_item(payload);
        let Some(card_id) = json_string(item, "cardId") else {
            return;
        };
        let Some(user_id) = json_string(item, "userId") else {
            return;
        };
        let Some(board) = self.subscribed_board_mut() else {
            return;
        };
        let Some(card) = find_card_mut(board, card_id) else {
            return;
        };
        card.assignees.retain(|user| user.id != user_id);
    }

    fn subscribed_board(&self) -> Option<&BoardSummary> {
        let target = self.subscribed_board_id.as_deref()?;
        self.projects
            .iter()
            .flat_map(|project| project.boards.iter())
            .find(|board| board.id == target)
    }

    fn subscribed_board_mut(&mut self) -> Option<&mut BoardSummary> {
        let board_id = self.subscribed_board_id.clone()?;
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
            self.set_selected(rows[0].key.clone());
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

    let initial_socket_session_id = 1;
    let (tx, rx) = mpsc::channel();
    // Only spawn a websocket listener at startup when the user explicitly
    // passed --board / PLNK_TUI_BOARD. Without that flag the TUI starts
    // idle and spawns the listener the first time a board is promoted to
    // the live target (TUI-012).
    let initial_socket_shutdown = args.board.clone().map(|board_id| {
        spawn_socket_listener(
            server.clone(),
            token.clone(),
            board_id,
            initial_socket_session_id,
            tx.clone(),
        )
    });

    if args.headless {
        if args.board.is_none() {
            println!(
                "headless probe: no --board supplied; \
                 nothing to subscribe to. Pass --board <id> or PLNK_TUI_BOARD to probe a board."
            );
            return Ok(());
        }
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
        initial_socket_session_id,
    );
    let runtime = Arc::new(tokio::runtime::Handle::current());

    let result = run_app(
        &mut terminal,
        &mut app,
        &rx,
        &tx,
        &server,
        &token,
        &runtime,
        initial_socket_shutdown,
    );
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

async fn fetch_project_tree(
    server: &Url,
    token: &str,
    project_id: &str,
) -> Result<ProjectTree, TuiError> {
    let client = reqwest::Client::new();
    let response = client
        .get(server.join(&format!("api/projects/{project_id}"))?)
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

    Ok(ProjectTree {
        id: snapshot.item.id,
        name: snapshot.item.name,
        description: snapshot.item.description,
        boards,
    })
}

async fn fetch_project_trees(
    server: &Url,
    token: &str,
    projects: &[ProjectSummary],
) -> Result<Vec<ProjectTree>, TuiError> {
    let mut trees = Vec::with_capacity(projects.len());

    for project in projects {
        trees.push(fetch_project_tree(server, token, &project.id).await?);
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

async fn fetch_card_comments(
    server: &Url,
    token: &str,
    card_id: &str,
) -> Result<Vec<CommentSummary>, TuiError> {
    let client = reqwest::Client::new();
    let response = client
        .get(server.join(&format!("api/cards/{card_id}/comments"))?)
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?;
    let response = response.json::<CommentsResponse>().await?;

    let users_by_id = response
        .included
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

    Ok(response
        .items
        .into_iter()
        .map(|comment| CommentSummary {
            id: comment.id,
            text: comment.text,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
            author: users_by_id.get(&comment.user_id).cloned(),
        })
        .collect::<Vec<_>>())
}

async fn update_card_fields(
    server: &Url,
    token: &str,
    card_id: &str,
    title: DraftFieldUpdate<String>,
    description: DraftFieldUpdate<String>,
) -> Result<Value, TuiError> {
    let client = reqwest::Client::new();
    let mut body = serde_json::Map::new();
    match title {
        DraftFieldUpdate::Set(title) => {
            body.insert("name".to_string(), Value::String(title));
        }
        DraftFieldUpdate::Unchanged | DraftFieldUpdate::Clear => {}
    }
    match description {
        DraftFieldUpdate::Set(description) => {
            body.insert("description".to_string(), Value::String(description));
        }
        DraftFieldUpdate::Clear => {
            body.insert("description".to_string(), Value::Null);
        }
        DraftFieldUpdate::Unchanged => {}
    }

    let response = client
        .patch(server.join(&format!("api/cards/{card_id}"))?)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<ItemResponse<Value>>().await?.item)
}

fn spawn_project_refresh(
    runtime: &Arc<tokio::runtime::Handle>,
    server: &Url,
    token: &str,
    project_id: String,
    loaded_board_ids: HashSet<String>,
    tx: &Sender<AppEvent>,
) {
    let runtime = Arc::clone(runtime);
    let server = server.clone();
    let token = token.to_string();
    let tx = tx.clone();
    runtime.spawn(async move {
        match fetch_project_tree(&server, &token, &project_id).await {
            Ok(mut project) => {
                let mut board_errors = Vec::new();
                for board in &mut project.boards {
                    if !loaded_board_ids.contains(&board.id) {
                        continue;
                    }
                    match fetch_board_summary(&server, &token, &board.id).await {
                        Ok(summary) => *board = summary,
                        Err(err) => board_errors.push((board.id.clone(), err.to_string())),
                    }
                }
                let _ = tx.send(AppEvent::ProjectRefreshed {
                    project,
                    board_errors,
                });
            }
            Err(err) => {
                let _ = tx.send(AppEvent::ProjectRefreshFailed {
                    project_id,
                    message: err.to_string(),
                });
            }
        }
    });
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

fn spawn_comment_loader(
    runtime: &Arc<tokio::runtime::Handle>,
    server: &Url,
    token: &str,
    card_id: String,
    tx: &Sender<AppEvent>,
) {
    let runtime = Arc::clone(runtime);
    let server = server.clone();
    let token = token.to_string();
    let tx = tx.clone();
    runtime.spawn(async move {
        match fetch_card_comments(&server, &token, &card_id).await {
            Ok(comments) => {
                let _ = tx.send(AppEvent::CardCommentsLoaded { card_id, comments });
            }
            Err(err) => {
                let _ = tx.send(AppEvent::CardCommentsLoadFailed {
                    card_id,
                    message: err.to_string(),
                });
            }
        }
    });
}

fn spawn_card_save(
    runtime: &Arc<tokio::runtime::Handle>,
    server: &Url,
    token: &str,
    card_id: String,
    title: DraftFieldUpdate<String>,
    description: DraftFieldUpdate<String>,
    tx: &Sender<AppEvent>,
) {
    let runtime = Arc::clone(runtime);
    let server = server.clone();
    let token = token.to_string();
    let tx = tx.clone();
    runtime.spawn(async move {
        match update_card_fields(&server, &token, &card_id, title, description).await {
            Ok(item) => {
                let _ = tx.send(AppEvent::CardSaveSucceeded { card_id, item });
            }
            Err(err) => {
                let _ = tx.send(AppEvent::CardSaveFailed {
                    card_id,
                    message: err.to_string(),
                });
            }
        }
    });
}

fn spawn_socket_listener(
    server: Url,
    token: String,
    board_id: String,
    session_id: u64,
    tx: Sender<AppEvent>,
) -> watch::Sender<bool> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        if let Err(err) =
            socket_task(server, token, board_id, session_id, tx.clone(), shutdown_rx).await
        {
            let _ = tx.send(AppEvent::SocketError {
                session_id,
                message: err.to_string(),
            });
        }
    });
    shutdown_tx
}

async fn socket_task(
    server: Url,
    token: String,
    board_id: String,
    session_id: u64,
    tx: Sender<AppEvent>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<(), TuiError> {
    let _ = tx.send(AppEvent::SocketConnecting {
        session_id,
        board_id: board_id.clone(),
    });

    let request = build_socket_request(&server)?;
    let (mut socket, _response) = connect_async(request)
        .await
        .map_err(|err| TuiError::Socket(format!("websocket connect failed: {err}")))?;

    let mut state = SocketSessionState::default();

    loop {
        let message = tokio::select! {
            changed = shutdown_rx.changed() => {
                match changed {
                    Ok(()) if *shutdown_rx.borrow() => {
                        let _ = socket.close(None).await;
                        return Ok(());
                    }
                    Ok(()) | Err(_) => continue,
                }
            }
            message = socket.next() => message,
        };

        let Some(message) = message else {
            break;
        };
        let message =
            message.map_err(|err| TuiError::Socket(format!("websocket read failed: {err}")))?;

        match message {
            Message::Text(text) => {
                handle_engine_text_message(
                    &text,
                    &mut socket,
                    &token,
                    &board_id,
                    session_id,
                    &tx,
                    &mut state,
                )
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
    session_id: u64,
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
            let _ = tx.send(AppEvent::LiveEvent {
                session_id,
                record: LiveEventRecord {
                    name: "engineOpen".to_string(),
                    summary: format!(
                        "sid={} pingInterval={} pingTimeout={}",
                        open.sid, open.ping_interval, open.ping_timeout
                    ),
                    payload: None,
                },
            });
            socket
                .send(Message::Text("40".into()))
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
                .send(Message::Text("3".into()))
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
                session_id,
                tx,
                &mut state.namespace_connected,
                &mut state.subscribe_sent,
            )
            .await?;
        }
        other => {
            let _ = tx.send(AppEvent::LiveEvent {
                session_id,
                record: LiveEventRecord {
                    name: "engineOther".to_string(),
                    summary: format!("type={other} raw={}", truncate(text, 120)),
                    payload: None,
                },
            });
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_socket_io_packet<S>(
    packet: &str,
    socket: &mut S,
    token: &str,
    board_id: &str,
    session_id: u64,
    tx: &Sender<AppEvent>,
    namespace_connected: &mut bool,
    subscribe_sent: &mut bool,
) -> Result<(), TuiError>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    if packet.starts_with('0') {
        *namespace_connected = true;
        let _ = tx.send(AppEvent::LiveEvent {
            session_id,
            record: LiveEventRecord {
                name: "socketConnect".to_string(),
                summary: truncate(packet, 120),
                payload: None,
            },
        });

        if !*subscribe_sent {
            let subscribe = build_subscribe_packet(token, board_id)?;
            socket
                .send(Message::Text(subscribe.into()))
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
            let _ = tx.send(AppEvent::SocketLive { session_id, board });
        }
        return Ok(());
    }

    if let Some((event_name, payload)) = parse_socket_event(packet)? {
        let _ = tx.send(AppEvent::LiveEvent {
            session_id,
            record: LiveEventRecord {
                name: event_name,
                summary: summarize_json(&payload),
                payload: Some(payload),
            },
        });
        return Ok(());
    }

    let _ = tx.send(AppEvent::LiveEvent {
        session_id,
        record: LiveEventRecord {
            name: "socketPacket".to_string(),
            summary: truncate(packet, 160),
            payload: None,
        },
    });

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

fn payload_card_id(payload: &Value) -> Option<&str> {
    let item = event_item(payload);
    json_string(item, "cardId").or_else(|| json_string(item, "id"))
}

fn payload_touches_card(payload: &Value, card_id: &str) -> bool {
    payload_card_id(payload) == Some(card_id)
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

fn find_card_mut<'a>(board: &'a mut BoardSummary, card_id: &str) -> Option<&'a mut CardSummary> {
    for list in &mut board.active_lists {
        if let Some(card) = list.cards.iter_mut().find(|card| card.id == card_id) {
            return Some(card);
        }
    }

    None
}

fn remove_card_from_board(board: &mut BoardSummary, card_id: &str) -> Option<CardSummary> {
    for list in &mut board.active_lists {
        if let Some(index) = list.cards.iter().position(|card| card.id == card_id) {
            let removed = list.cards.remove(index);
            list.card_count = list.cards.len();
            list.active_card_count = list.cards.iter().filter(|card| !card.is_closed).count();
            list.closed_card_count = list.card_count.saturating_sub(list.active_card_count);
            return Some(removed);
        }
    }

    None
}

fn recount_board(board: &mut BoardSummary) {
    for list in &mut board.active_lists {
        list.card_count = list.cards.len();
        list.active_card_count = list.cards.iter().filter(|card| !card.is_closed).count();
        list.closed_card_count = list.card_count.saturating_sub(list.active_card_count);
        list.cards
            .sort_by(|left, right| left.position.total_cmp(&right.position));
    }
    board.total_cards = board.active_lists.iter().map(|list| list.cards.len()).sum();
    board.active_card_count = board
        .active_lists
        .iter()
        .map(|list| list.active_card_count)
        .sum();
    board.closed_card_count = board.total_cards.saturating_sub(board.active_card_count);
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

fn filter_matches(query: &str, label: &str) -> bool {
    let query = query.to_lowercase();
    let label = label.to_lowercase();
    if query.contains(['*', '?']) {
        glob_matches(&query, &label)
    } else {
        label.contains(&query)
    }
}

fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    let mut dp = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;

    for i in 0..pattern.len() {
        match pattern[i] {
            '*' => {
                dp[i + 1][0] = dp[i][0];
                for j in 0..text.len() {
                    dp[i + 1][j + 1] = dp[i][j + 1] || dp[i + 1][j];
                }
            }
            '?' => {
                for j in 0..text.len() {
                    dp[i + 1][j + 1] = dp[i][j];
                }
            }
            ch => {
                for j in 0..text.len() {
                    dp[i + 1][j + 1] = dp[i][j] && ch == text[j];
                }
            }
        }
    }

    dp[pattern.len()][text.len()]
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

fn edit_description_via_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    current: Option<&str>,
    card_id: &str,
) -> Result<Option<String>, TuiError> {
    let path = temp_editor_path(card_id);
    fs::write(&path, current.unwrap_or_default())?;

    restore_terminal()?;
    let editor_cmd = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let command = format!("{} {}", editor_cmd, shell_escape(&path));
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .status()
        .map_err(TuiError::Io)?;
    *terminal = init_terminal()?;

    let text = fs::read_to_string(&path)?;
    let _ = fs::remove_file(&path);

    if !status.success() {
        return Err(TuiError::Io(io::Error::other(
            "$EDITOR exited unsuccessfully",
        )));
    }

    if current.is_some_and(|existing| text == existing || text == format!("{existing}\n")) {
        return Ok(current.map(ToOwned::to_owned));
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

fn temp_editor_path(card_id: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    env::temp_dir().join(format!("plnk-tui-card-{card_id}-{stamp}.md"))
}

fn shell_escape(path: &Path) -> String {
    let text = path.to_string_lossy();
    format!("'{}'", text.replace('\'', "'\\''"))
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
            Ok(AppEvent::SocketConnecting { board_id, .. }) => {
                println!("event: socket connecting: {board_id}");
            }
            Ok(AppEvent::ProjectRefreshed { project, .. }) => println!(
                "event: project refreshed: {} ({}) :: {} boards",
                project.name,
                project.id,
                project.boards.len()
            ),
            Ok(AppEvent::ProjectRefreshFailed {
                project_id,
                message,
            }) => {
                println!("event: project refresh failed: {project_id} :: {message}");
            }
            Ok(AppEvent::SocketLive { board, .. } | AppEvent::BoardHydrated(board)) => println!(
                "event: board live: {} [{}] cards={} lists={}",
                board.name,
                board.id,
                board.total_cards,
                board.active_lists.len()
            ),
            Ok(AppEvent::BoardLoadFailed { board_id, message }) => {
                println!("event: board load failed: {board_id} :: {message}");
            }
            Ok(AppEvent::CardCommentsLoaded { card_id, comments }) => {
                println!("event: comments loaded: {card_id} :: {}", comments.len());
            }
            Ok(AppEvent::CardCommentsLoadFailed { card_id, message }) => {
                println!("event: comments load failed: {card_id} :: {message}");
            }
            Ok(AppEvent::CardSaveSucceeded { card_id, .. }) => {
                println!("event: card saved: {card_id}");
            }
            Ok(AppEvent::CardSaveFailed { card_id, message }) => {
                println!("event: card save failed: {card_id} :: {message}");
            }
            Ok(AppEvent::SocketError { message, .. }) => {
                println!("event: socket error: {message}");
            }
            Ok(AppEvent::LiveEvent { record, .. }) => {
                println!("event: {} :: {}", record.name, record.summary);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
    rx: &Receiver<AppEvent>,
    tx: &Sender<AppEvent>,
    server: &Url,
    token: &str,
    runtime: &Arc<tokio::runtime::Handle>,
    mut socket_shutdown: Option<watch::Sender<bool>>,
) -> Result<(), TuiError> {
    loop {
        while let Ok(message) = rx.try_recv() {
            app.apply(message);
        }
        app.flush_pending_save_completion();

        if let Some(card_id) = app.mark_selected_card_comments_loading() {
            spawn_comment_loader(runtime, server, token, card_id, tx);
        }

        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                CEvent::Key(key) if key.kind == KeyEventKind::Press => {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        return Ok(());
                    }

                    if app.saving_card.is_some() {
                        app.set_notice("Save in progress… please wait.");
                        continue;
                    }

                    if app.title_editor.is_some() {
                        match key.code {
                            KeyCode::Esc => app.cancel_title_editor(),
                            KeyCode::Enter => {
                                app.apply_title_editor();
                                if let Some((card_id, title, description)) = app.save_request() {
                                    spawn_card_save(
                                        runtime,
                                        server,
                                        token,
                                        card_id,
                                        title,
                                        description,
                                        tx,
                                    );
                                } else if !app.has_dirty_card_draft() {
                                    app.set_notice("No title changes to save.");
                                }
                            }
                            KeyCode::Left => app.edit_title_move_left(),
                            KeyCode::Right => app.edit_title_move_right(),
                            KeyCode::Home => app.edit_title_move_home(),
                            KeyCode::End => app.edit_title_move_end(),
                            KeyCode::Backspace => app.edit_title_backspace(),
                            KeyCode::Delete => app.edit_title_delete(),
                            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.edit_title_insert(ch);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if app.filter_editor.is_some() {
                        match key.code {
                            KeyCode::Esc => app.clear_or_close_filter_editor(),
                            KeyCode::Enter => app.finish_filter_editor(),
                            KeyCode::Left => app.edit_filter_move_left(),
                            KeyCode::Right => app.edit_filter_move_right(),
                            KeyCode::Home => app.edit_filter_move_home(),
                            KeyCode::End => app.edit_filter_move_end(),
                            KeyCode::Backspace => app.edit_filter_backspace(),
                            KeyCode::Delete => app.edit_filter_delete(),
                            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.edit_filter_insert(ch);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc if app.has_dirty_card_draft() => {
                            app.set_notice("Unsaved card changes. Ctrl-s save • Ctrl-x discard.");
                        }
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Tab | KeyCode::BackTab => app.toggle_focus(),
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Some((card_id, title, description)) = app.save_request() {
                                spawn_card_save(
                                    runtime,
                                    server,
                                    token,
                                    card_id,
                                    title,
                                    description,
                                    tx,
                                );
                            }
                        }
                        KeyCode::Char('x')
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                && app.has_dirty_card_draft() =>
                        {
                            app.discard_card_draft();
                            app.set_notice("Discarded local card edits.");
                        }
                        KeyCode::Char('/') => {
                            app.start_filter_editor();
                        }
                        KeyCode::Char('v') if app.has_dirty_card_draft() => {
                            app.set_notice(
                                "Save or discard dirty card before changing explorer view.",
                            );
                        }
                        KeyCode::Char('v') => {
                            app.toggle_explorer_view();
                        }
                        KeyCode::Char('r' | 'R')
                            if !key.modifiers.contains(KeyModifiers::CONTROL)
                                && app.has_dirty_card_draft() =>
                        {
                            app.set_notice(
                                "Save or discard dirty card before refreshing hierarchy.",
                            );
                        }
                        KeyCode::Char('r' | 'R')
                            if !key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            if let Some(target) = app.queue_hierarchy_refresh() {
                                match target {
                                    HierarchyRefreshTarget::Project {
                                        project_id,
                                        loaded_board_ids,
                                    } => {
                                        spawn_project_refresh(
                                            runtime,
                                            server,
                                            token,
                                            project_id,
                                            loaded_board_ids,
                                            tx,
                                        );
                                    }
                                    HierarchyRefreshTarget::Board {
                                        board_id,
                                        comment_card_id,
                                    } => {
                                        spawn_board_loader(runtime, server, token, board_id, tx);
                                        if let Some(card_id) = comment_card_id {
                                            spawn_comment_loader(
                                                runtime, server, token, card_id, tx,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('e') => {
                            if app.selected_card_id().is_some() {
                                app.start_title_editor();
                            } else {
                                app.set_notice("Select card first to edit title.");
                            }
                        }
                        KeyCode::Char('E') => {
                            if let Some(card_id) = app.selected_card_id().map(ToOwned::to_owned) {
                                let current = app
                                    .ensure_card_draft()
                                    .and_then(|draft| draft.draft_description.clone());
                                match edit_description_via_editor(
                                    terminal,
                                    current.as_deref(),
                                    &card_id,
                                ) {
                                    Ok(description) => {
                                        if let Some(draft) = app.ensure_card_draft() {
                                            draft.draft_description.clone_from(&description);
                                        }
                                        if description == current {
                                            app.set_notice("No description changes from $EDITOR.");
                                        } else if let Some((card_id, title, description)) =
                                            app.save_request()
                                        {
                                            spawn_card_save(
                                                runtime,
                                                server,
                                                token,
                                                card_id,
                                                title,
                                                description,
                                                tx,
                                            );
                                        } else if !app.has_dirty_card_draft() {
                                            app.set_notice("No description changes to save.");
                                        }
                                    }
                                    Err(err) => app.set_notice(err.to_string()),
                                }
                            } else {
                                app.set_notice("Select card first to edit description.");
                            }
                        }
                        KeyCode::Char('L') => {
                            if let Some(board_id) = app.selected_board_id().map(ToOwned::to_owned) {
                                if app.is_live_target(&board_id) {
                                    if let Some(previous) = socket_shutdown.take() {
                                        let _ = previous.send(true);
                                    }
                                    app.clear_live_board();
                                } else {
                                    if let Some(previous) = socket_shutdown.take() {
                                        let _ = previous.send(true);
                                    }
                                    app.switch_live_board(&board_id);
                                    socket_shutdown = Some(spawn_socket_listener(
                                        server.clone(),
                                        token.to_string(),
                                        board_id,
                                        app.active_socket_session_id,
                                        tx.clone(),
                                    ));
                                }
                            } else {
                                app.set_notice("Select a board to make it live.");
                            }
                        }
                        KeyCode::Char('g') if app.focus == PaneFocus::Details => {
                            app.scroll_details_to_top();
                        }
                        KeyCode::Char('G') if app.focus == PaneFocus::Details => {
                            app.scroll_details_to_bottom();
                        }
                        KeyCode::PageDown if app.focus == PaneFocus::Details => {
                            app.scroll_details_by(10);
                        }
                        KeyCode::PageUp if app.focus == PaneFocus::Details => {
                            app.scroll_details_by(-10);
                        }
                        KeyCode::Char('d')
                            if app.focus == PaneFocus::Details
                                && key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.scroll_details_by(10);
                        }
                        KeyCode::Char('u')
                            if app.focus == PaneFocus::Details
                                && key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.scroll_details_by(-10);
                        }
                        KeyCode::Char('d' | 'D')
                            if !key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.toggle_debug_log();
                        }
                        KeyCode::Char('y') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Some(payload) = app.fast_copy() {
                                match write_osc52_clipboard(&payload.json) {
                                    Ok(()) => app.set_notice(format!(
                                        "Copied JSON → clipboard: {}",
                                        payload.breadcrumb
                                    )),
                                    Err(err) => {
                                        app.set_notice(format!("Copy failed: {err}"));
                                    }
                                }
                            } else {
                                app.set_notice("Select a node to copy its ID hierarchy.");
                            }
                        }
                        KeyCode::Char('Y') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Some(payload) = app.fast_copy() {
                                match write_osc52_clipboard(&payload.command) {
                                    Ok(()) => app.set_notice(format!(
                                        "Copied snapshot command → clipboard: {}",
                                        payload.breadcrumb
                                    )),
                                    Err(err) => {
                                        app.set_notice(format!("Copy failed: {err}"));
                                    }
                                }
                            } else {
                                app.set_notice("Select a node to copy its ID hierarchy.");
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => match app.focus {
                            PaneFocus::Explorer if app.has_dirty_card_draft() => {
                                app.set_notice(
                                    "Save or discard dirty card before changing selection.",
                                );
                            }
                            PaneFocus::Explorer => app.select_relative(1),
                            PaneFocus::Details => app.scroll_details_by(1),
                        },
                        KeyCode::Up | KeyCode::Char('k') => match app.focus {
                            PaneFocus::Explorer if app.has_dirty_card_draft() => {
                                app.set_notice(
                                    "Save or discard dirty card before changing selection.",
                                );
                            }
                            PaneFocus::Explorer => app.select_relative(-1),
                            PaneFocus::Details => app.scroll_details_by(-1),
                        },
                        KeyCode::Left | KeyCode::Char('h') => match app.focus {
                            PaneFocus::Explorer if app.has_dirty_card_draft() => {
                                app.set_notice(
                                    "Save or discard dirty card before changing selection.",
                                );
                            }
                            PaneFocus::Explorer => app.collapse_or_ascend(),
                            PaneFocus::Details => app.focus_explorer(),
                        },
                        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => match app.focus {
                            PaneFocus::Explorer if app.has_dirty_card_draft() => {
                                app.set_notice(
                                    "Save or discard dirty card before changing selection.",
                                );
                            }
                            PaneFocus::Explorer => {
                                if let Some(board_id) = app.expand_or_descend() {
                                    spawn_board_loader(runtime, server, token, board_id, tx);
                                }
                            }
                            PaneFocus::Details => app.focus_details(),
                        },
                        _ => {}
                    }
                }
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
            Constraint::Length(1),
        ])
        .split(area);

    let mut header_title = vec![
        Span::styled(
            "plnk-tui explorer",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  •  "),
        Span::styled(app.status.label(), app.status.style()),
    ];
    if app.title_editor.is_some() {
        header_title.push(Span::raw("  •  "));
        header_title.push(chip_span("EDITING TITLE", Color::LightYellow));
    }
    if app.filter_editor.is_some() {
        header_title.push(Span::raw("  •  "));
        header_title.push(chip_span("FILTER MODE", Color::Magenta));
    } else if app.has_active_filter() {
        header_title.push(Span::raw("  •  "));
        header_title.push(chip_span("FILTER", Color::Magenta));
    }
    if app.has_dirty_card_draft() {
        header_title.push(Span::raw("  •  "));
        header_title.push(chip_span("DIRTY", Color::Yellow));
    }
    if app
        .card_draft
        .as_ref()
        .is_some_and(|draft| draft.remote_changed && draft.is_dirty())
    {
        header_title.push(Span::raw("  •  "));
        header_title.push(chip_span("REMOTE CHANGED", Color::LightRed));
    }
    if app.saving_card.is_some() {
        header_title.push(Span::raw("  •  "));
        header_title.push(chip_span("SAVING", Color::LightBlue));
    }

    let header_lines = vec![
        Line::from(header_title),
        Line::from(format!(
            "server: {} | login: {} | current user: {} ({})",
            app.server, app.login, app.current_user.name, app.current_user.username
        )),
        Line::from(format!(
            "visible projects: {} | current user id: {} | explorer view: {} | filter: {} | live target: {}",
            app.projects.len(),
            app.current_user.id,
            app.explorer_view.label(),
            if app.has_active_filter() {
                truncate(app.active_filter_query(), 30)
            } else {
                "none".to_string()
            },
            app.subscribed_board_id
                .as_deref()
                .unwrap_or("none (press L on a board)")
        )),
    ];
    frame.render_widget(
        Paragraph::new(header_lines)
            .block(panel_block("session", false))
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
        vec![ListItem::new(if app.has_active_filter() {
            format!(
                "No nodes match filter '{}'",
                truncate(app.active_filter_query(), 60)
            )
        } else {
            "No projects visible for this user.".to_string()
        })]
    } else {
        rows.iter().map(render_tree_row).collect::<Vec<_>>()
    };

    let mut tree_state = ListState::default();
    if !rows.is_empty() {
        tree_state.select(Some(selected_index));
    }

    let tree = List::new(tree_items)
        .block(panel_block(
            &if app.has_active_filter() {
                format!(
                    "explorer • {} • filter: {}",
                    app.explorer_view.label(),
                    truncate(app.active_filter_query(), 24)
                )
            } else {
                format!("explorer • {}", app.explorer_view.label())
            },
            app.focus == PaneFocus::Explorer,
        ))
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
            .block(panel_block("details", app.focus == PaneFocus::Details))
            .scroll((u16::try_from(app.detail_scroll).unwrap_or(u16::MAX), 0))
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
            "live target: {}",
            app.board.as_ref().map_or_else(
                || {
                    app.subscribed_board_id.as_deref().map_or_else(
                        || "none — select a board and press L to promote it".to_string(),
                        |id| format!("{id} (waiting for snapshot)"),
                    )
                },
                |board| format!("{} [{}]", board.name, board.id),
            )
        )),
        Line::from(format!("latest event: {}", app.latest_event_summary())),
        Line::from(format!(
            "notice: {}",
            app.notice.as_deref().unwrap_or("none")
        )),
        Line::from(""),
        Line::from("tip: press D to toggle the websocket debug log overlay"),
    ];
    frame.render_widget(
        Paragraph::new(live_lines)
            .block(panel_block("live sync", false))
            .wrap(Wrap { trim: true }),
        right[1],
    );

    let key_help = if app.saving_card.is_some() {
        "SAVING: waiting for server response • controls paused • Ctrl-c force quit"
    } else if app.title_editor.is_some() {
        "TITLE MODE: type text • ←/→ move • Enter save • Esc cancel • Ctrl-c force quit"
    } else if app.filter_editor.is_some() {
        "FILTER MODE: type text • * and ? globs • Enter keep • Esc clear/close • Ctrl-c force quit"
    } else {
        "↑/↓ nav • / filter • →/Enter expand • r refresh • v toggle view • L live on/off • e edit title • E edit description ($EDITOR) • y copy JSON • Y copy cmd • D debug log • Ctrl-c quit"
    };
    frame.render_widget(
        Paragraph::new(key_help).style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );

    if app.show_debug_log {
        draw_debug_overlay(frame, area, app);
    }
    if app.filter_editor.is_some() {
        draw_filter_editor_overlay(frame, area, app);
    }
    if app.title_editor.is_some() {
        draw_title_editor_overlay(frame, area, app);
    }
    if app.saving_card.is_some() {
        draw_saving_overlay(frame, area, app);
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
        TreeKind::LabelGroup => "◌",
        TreeKind::Card => "·",
    };
    let accent = match row.kind {
        TreeKind::Project => Color::Cyan,
        TreeKind::Board => Color::LightGreen,
        TreeKind::List => Color::Yellow,
        TreeKind::LabelGroup => Color::Magenta,
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
        TreeKey::LabelGroup {
            board_id,
            list_id,
            label_id,
        } => build_label_group_detail(app, board_id, list_id, label_id.as_deref()),
        TreeKey::Card(card_id) | TreeKey::GroupedCard { card_id, .. } => {
            build_card_detail(app, card_id)
        }
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
        header_value_line(&project.name),
        id_line("project", &project.id),
        Line::from(""),
        section_header("Summary"),
        kv_line(
            "boards",
            format!("{} total • {} loaded", project.boards.len(), loaded_boards),
        ),
        Line::from(""),
        section_header("Description"),
    ];

    push_optional_text_block(
        &mut lines,
        project.description.as_deref(),
        "No project description.",
    );
    lines.push(Line::from(""));
    lines.push(section_header("Boards"));
    lines.extend(project.boards.iter().take(12).map(|board| {
        let status = if app.is_live_connected(&board.id) {
            "live"
        } else if app.is_live_target(&board.id) {
            "live target"
        } else if board.is_loaded() {
            "loaded"
        } else {
            "stub"
        };
        bullet_with_meta(&board.name, &format!("{status} • {}", board.id))
    }));
    lines
}

fn build_board_detail(app: &AppState, board_id: &str) -> Vec<Line<'static>> {
    for project in &app.projects {
        if let Some(board) = project.boards.iter().find(|board| board.id == board_id) {
            let mut lines = vec![
                detail_title("board"),
                header_value_line(&board.name),
                id_line("board", &board.id),
                context_entity_line("project", &project.name, &project.id),
                Line::from(""),
                section_header("Summary"),
                kv_line(
                    "status",
                    if app.is_live_connected(&board.id) {
                        "live subscribed".to_string()
                    } else if app.is_live_target(&board.id) {
                        "switching live target".to_string()
                    } else {
                        "loaded by snapshot".to_string()
                    },
                ),
            ];

            if board.is_loaded() {
                lines.push(kv_line(
                    "cards",
                    format!(
                        "{} active • {} closed",
                        board.active_card_count, board.closed_card_count
                    ),
                ));
                lines.push(kv_line("lists", board.active_lists.len().to_string()));
                lines.push(kv_line("labels", board.labels.len().to_string()));
                lines.push(kv_line("members", board.members.len().to_string()));

                lines.push(Line::from(""));
                lines.push(section_header("Lists"));
                lines.extend(board.active_lists.iter().take(10).map(|list| {
                    bullet_with_meta(
                        &list.name,
                        &format!(
                            "{} active • {} closed • {}",
                            list.active_card_count, list.closed_card_count, list.id
                        ),
                    )
                }));

                lines.push(Line::from(""));
                lines.push(section_header("Labels"));
                if board.labels.is_empty() {
                    lines.push(empty_state_line("No labels."));
                } else {
                    lines.extend(board.labels.iter().take(8).map(|label| {
                        bullet_with_meta(
                            &label.name,
                            &format!(
                                "color={} • {}",
                                label.color.as_deref().unwrap_or("?"),
                                label.id
                            ),
                        )
                    }));
                }

                lines.push(Line::from(""));
                lines.push(section_header("Members"));
                lines.extend(board.members.iter().take(8).map(|member| {
                    bullet_with_meta(
                        &member.name,
                        &format!("@{} • {}", member.username, member.id),
                    )
                }));
            } else {
                lines.push(Line::from(""));
                lines.push(section_header("Load state"));
                lines.push(muted_line(
                    "Board known from project snapshot, but richer list/card detail not loaded yet.",
                ));
                lines.push(muted_line(
                    "Press → or Enter to lazy-load this board snapshot over HTTP.",
                ));
                lines.push(muted_line(
                    "Press L on a board node to toggle websocket live sync for that board.",
                ));
            }

            return lines;
        }
    }

    vec![Line::from("Selected board is no longer available.")]
}

fn build_label_group_detail(
    app: &AppState,
    board_id: &str,
    list_id: &str,
    label_id: Option<&str>,
) -> Vec<Line<'static>> {
    for project in &app.projects {
        if let Some(board) = project.boards.iter().find(|board| board.id == board_id) {
            if let Some(list) = board.active_lists.iter().find(|list| list.id == list_id) {
                let groups = label_groups_for_list(board, list);
                let Some(group) = groups
                    .iter()
                    .find(|group| group.label_id.as_deref() == label_id)
                else {
                    return vec![Line::from("Selected label group is no longer available.")];
                };

                let mut lines = vec![
                    detail_title("label group"),
                    header_value_line(&group.label_name),
                    context_entity_line("list", &list.name, &list.id),
                    context_entity_line("board", &board.name, &board.id),
                    context_entity_line("project", &project.name, &project.id),
                    Line::from(""),
                    section_header("Summary"),
                    kv_line(
                        "cards",
                        format!(
                            "{} active • {} closed • {} total",
                            group.active_card_count,
                            group.closed_card_count,
                            group.cards.len()
                        ),
                    ),
                ];

                if let Some(label_id) = &group.label_id {
                    lines.push(kv_line("label id", label_id.clone()));
                    lines.push(kv_line(
                        "color",
                        group.color.clone().unwrap_or_else(|| "none".to_string()),
                    ));
                } else {
                    lines.push(kv_line("bucket", "cards without labels".to_string()));
                }

                lines.push(Line::from(""));
                lines.push(section_header("Cards"));
                if group.cards.is_empty() {
                    lines.push(empty_state_line("No cards in this label group."));
                } else {
                    lines.extend(group.cards.iter().take(16).map(|card| {
                        bullet_with_meta(
                            &card.card_name,
                            &format!(
                                "{}{} • {}",
                                card.list_name,
                                if card.is_closed { " • closed" } else { "" },
                                card.card_id
                            ),
                        )
                    }));
                }

                return lines;
            }
        }
    }

    vec![Line::from("Selected label group is no longer available.")]
}

fn build_list_detail(app: &AppState, list_id: &str) -> Vec<Line<'static>> {
    for project in &app.projects {
        for board in &project.boards {
            if let Some(list) = board.active_lists.iter().find(|list| list.id == list_id) {
                let mut lines = vec![
                    detail_title("list"),
                    header_value_line(&list.name),
                    id_line("list", &list.id),
                    context_entity_line("board", &board.name, &board.id),
                    context_entity_line("project", &project.name, &project.id),
                    Line::from(""),
                    section_header("Summary"),
                    kv_line(
                        "cards",
                        format!(
                            "{} total • {} active • {} closed",
                            list.card_count, list.active_card_count, list.closed_card_count
                        ),
                    ),
                    Line::from(""),
                    section_header("Cards in list"),
                ];
                lines.extend(list.cards.iter().take(14).map(|card| {
                    let mut meta = Vec::new();
                    if !card.labels.is_empty() {
                        meta.push(join_label_names(&card.labels));
                    }
                    if card.is_closed {
                        meta.push("closed".to_string());
                    }
                    meta.push(card.id.clone());
                    bullet_with_meta(&card.name, &meta.join(" • "))
                }));
                return lines;
            }
        }
    }

    vec![Line::from("Selected list is no longer available.")]
}

#[allow(clippy::too_many_lines)]
fn build_card_detail(app: &AppState, card_id: &str) -> Vec<Line<'static>> {
    for project in &app.projects {
        for board in &project.boards {
            for list in &board.active_lists {
                if let Some(card) = list.cards.iter().find(|card| card.id == card_id) {
                    let draft = app.current_card_draft(card_id);
                    let title =
                        draft.map_or(card.name.as_str(), |draft| draft.draft_title.as_str());
                    let description = draft
                        .and_then(|draft| draft.draft_description.as_deref())
                        .or(card.description.as_deref());
                    let status = if card.is_closed { "CLOSED" } else { "ACTIVE" };
                    let due = card.due_date.as_deref().unwrap_or("no due date");
                    let subscribed = if card.is_subscribed {
                        "subscribed"
                    } else {
                        "not subscribed"
                    };

                    let mut chips = vec![
                        chip_span(
                            status,
                            if card.is_closed {
                                Color::Yellow
                            } else {
                                Color::LightGreen
                            },
                        ),
                        Span::raw("  "),
                        chip_span(due, Color::LightBlue),
                        Span::raw("  "),
                        chip_span(subscribed, Color::Gray),
                    ];
                    if draft.is_some_and(CardDraft::is_dirty) {
                        chips.push(Span::raw("  "));
                        chips.push(chip_span("DIRTY", Color::Yellow));
                    }
                    if draft.is_some_and(|draft| draft.remote_changed && draft.is_dirty()) {
                        chips.push(Span::raw("  "));
                        chips.push(chip_span("REMOTE CHANGED", Color::LightRed));
                    }
                    if app.saving_card.as_deref() == Some(card_id) {
                        chips.push(Span::raw("  "));
                        chips.push(chip_span("SAVING", Color::LightBlue));
                    }

                    let mut lines = vec![
                        detail_title("card"),
                        header_value_line(title),
                        Line::from(chips),
                        id_line("card", &card.id),
                        Line::from(""),
                        section_header("Context"),
                        context_entity_line("list", &list.name, &list.id),
                        context_entity_line("board", &board.name, &board.id),
                        context_entity_line("project", &project.name, &project.id),
                        Line::from(""),
                        section_header("Metadata"),
                        kv_line(
                            "creator",
                            card.creator.as_ref().map_or_else(
                                || "unknown".to_string(),
                                |creator| format!("{} (@{})", creator.name, creator.username),
                            ),
                        ),
                        kv_line(
                            "labels",
                            if card.labels.is_empty() {
                                "none".to_string()
                            } else {
                                join_label_names(&card.labels)
                            },
                        ),
                        kv_line(
                            "assignees",
                            if card.assignees.is_empty() {
                                "none".to_string()
                            } else {
                                join_user_names(&card.assignees)
                            },
                        ),
                        kv_line("comments", card.comments_total.to_string()),
                        Line::from(""),
                        section_header("Description"),
                    ];
                    push_optional_text_block(&mut lines, description, "No card description.");

                    lines.push(Line::from(""));
                    lines.push(section_header("Attachments"));
                    if card.attachments.is_empty() {
                        lines.push(empty_state_line("No attachments."));
                    } else {
                        for attachment in &card.attachments {
                            lines.push(bullet_with_meta(&attachment.name, &attachment.id));
                            if let Some(url) = &attachment.url {
                                lines
                                    .push(muted_indented_line(&format!("↳ {}", truncate(url, 72))));
                            }
                        }
                    }

                    lines.push(Line::from(""));
                    lines.push(section_header("Tasks"));
                    if card.task_lists.is_empty() {
                        lines.push(empty_state_line("No task lists."));
                    } else {
                        for task_list in &card.task_lists {
                            let completed = task_list
                                .tasks
                                .iter()
                                .filter(|task| task.is_completed)
                                .count();
                            lines.push(bullet_with_meta(
                                &task_list.name,
                                &format!(
                                    "{}/{} • {}",
                                    completed,
                                    task_list.tasks.len(),
                                    task_list.id
                                ),
                            ));
                            for task in &task_list.tasks {
                                let marker = if task.is_completed { "[x]" } else { "[ ]" };
                                let assignee = task
                                    .assignee
                                    .as_ref()
                                    .map_or(String::new(), |user| format!(" • @{}", user.username));
                                lines.push(muted_indented_line(&format!(
                                    "{} {}{} • {}",
                                    marker, task.name, assignee, task.id
                                )));
                            }
                        }
                    }

                    lines.push(Line::from(""));
                    lines.push(section_header("Comments"));
                    match (
                        app.card_comments.get(card_id),
                        app.loading_card_comments.contains(card_id),
                        app.card_comment_errors.get(card_id),
                    ) {
                        (Some(comments), _, _) if comments.is_empty() => {
                            lines.push(empty_state_line("No comments."));
                        }
                        (Some(comments), _, _) => {
                            for comment in comments {
                                let author = comment.author.as_ref().map_or_else(
                                    || "unknown".to_string(),
                                    |author| format!("{} (@{})", author.name, author.username),
                                );
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        author,
                                        Style::default()
                                            .fg(Color::White)
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                    Span::raw("  "),
                                    Span::styled(
                                        truncate(&comment.created_at, 32),
                                        Style::default().fg(Color::DarkGray),
                                    ),
                                ]));
                                lines.push(muted_indented_line(&format!("id: {}", comment.id)));
                                push_prefixed_text_block(&mut lines, &comment.text, "    ");
                                if let Some(updated_at) = &comment.updated_at {
                                    lines.push(muted_indented_line(&format!(
                                        "updated: {updated_at}"
                                    )));
                                }
                                lines.push(Line::from(""));
                            }
                            let _ = lines.pop();
                        }
                        (None, true, _) => {
                            lines.push(empty_state_line("Loading comments…"));
                        }
                        (None, _, Some(message)) => {
                            lines.push(empty_state_line(&format!(
                                "Failed to load comments: {message}"
                            )));
                        }
                        (None, false, None) => {
                            lines.push(empty_state_line(
                                "Comments will load when card is selected.",
                            ));
                        }
                    }

                    lines.push(Line::from(""));
                    lines.push(section_header("Editing"));
                    lines.push(muted_line(
                        "e edits title inline • E opens description in $EDITOR and saves on exit • Ctrl-s saves • Ctrl-x discards",
                    ));
                    if app.saving_card.as_deref() == Some(card_id) {
                        lines.push(muted_line(
                            "Saving current card changes to Planka server… controls are paused until response.",
                        ));
                    } else if let Some(draft) = draft.filter(|draft| draft.is_dirty()) {
                        lines.push(muted_line(&format!(
                            "Local draft differs from base snapshot.{}",
                            if draft.remote_changed {
                                " Remote changes also arrived while dirty."
                            } else {
                                ""
                            }
                        )));
                    }
                    return lines;
                }
            }
        }
    }

    vec![Line::from("Selected card is no longer available.")]
}

fn header_value_line(value: &str) -> Line<'static> {
    Line::from(Span::styled(
        value.to_string(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ))
}

fn section_header(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("─ {title} ─"),
        Style::default()
            .fg(Color::LightCyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn key_span(label: &str) -> Span<'static> {
    Span::styled(
        format!("{label}: "),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
}

fn value_span(value: &str) -> Span<'static> {
    Span::styled(value.to_string(), Style::default().fg(Color::Gray))
}

fn dim_span(value: &str) -> Span<'static> {
    Span::styled(value.to_string(), Style::default().fg(Color::DarkGray))
}

fn kv_line(label: &str, value: impl std::fmt::Display) -> Line<'static> {
    let value = format!("{value}");
    Line::from(vec![key_span(label), value_span(&value)])
}

fn id_line(kind: &str, id: &str) -> Line<'static> {
    Line::from(vec![dim_span(&format!("{kind} id: {id}"))])
}

fn context_entity_line(label: &str, name: &str, id: &str) -> Line<'static> {
    Line::from(vec![
        key_span(label),
        Span::styled(name.to_string(), Style::default().fg(Color::White)),
        Span::raw("  "),
        dim_span(&format!("[{id}]")),
    ])
}

fn chip_span(text: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!(" {text} "),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn bullet_with_meta(name: &str, meta: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled("• ", Style::default().fg(Color::DarkGray)),
        Span::styled(name.to_string(), Style::default().fg(Color::White)),
        Span::raw("  "),
        dim_span(meta),
    ])
}

fn empty_state_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::DarkGray),
    ))
}

fn muted_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::Gray),
    ))
}

fn muted_indented_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("    {text}"),
        Style::default().fg(Color::Gray),
    ))
}

fn label_group_key(board_id: &str, list_id: &str, label_id: Option<&str>) -> String {
    match label_id {
        Some(label_id) => format!("{board_id}::{list_id}::{label_id}"),
        None => format!("{board_id}::{list_id}::__unlabeled"),
    }
}

fn label_groups_for_list(board: &BoardSummary, list: &ListSummary) -> Vec<LabelGroupSummary> {
    let mut groups = board
        .labels
        .iter()
        .map(|label| LabelGroupSummary {
            board_id: board.id.clone(),
            list_id: list.id.clone(),
            label_id: Some(label.id.clone()),
            label_name: label.name.clone(),
            color: label.color.clone(),
            active_card_count: 0,
            closed_card_count: 0,
            cards: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut unlabeled = LabelGroupSummary {
        board_id: board.id.clone(),
        list_id: list.id.clone(),
        label_id: None,
        label_name: "Unlabeled".to_string(),
        color: None,
        active_card_count: 0,
        closed_card_count: 0,
        cards: Vec::new(),
    };

    for card in &list.cards {
        let grouped_card = LabelGroupCard {
            card_id: card.id.clone(),
            card_name: card.name.clone(),
            list_id: list.id.clone(),
            list_name: list.name.clone(),
            card_position: card.position,
            is_closed: card.is_closed,
        };

        if card.labels.is_empty() {
            if card.is_closed {
                unlabeled.closed_card_count += 1;
            } else {
                unlabeled.active_card_count += 1;
            }
            unlabeled.cards.push(grouped_card);
            continue;
        }

        for card_label in &card.labels {
            if let Some(group) = groups
                .iter_mut()
                .find(|group| group.label_id.as_deref() == Some(card_label.id.as_str()))
            {
                if card.is_closed {
                    group.closed_card_count += 1;
                } else {
                    group.active_card_count += 1;
                }
                group.cards.push(grouped_card.clone());
            }
        }
    }

    groups.retain(|group| !group.cards.is_empty());
    if !unlabeled.cards.is_empty() {
        groups.push(unlabeled);
    }

    for group in &mut groups {
        group.cards.sort_by(|left, right| {
            left.card_position
                .total_cmp(&right.card_position)
                .then_with(|| left.card_name.cmp(&right.card_name))
                .then_with(|| left.list_id.cmp(&right.list_id))
        });
    }

    groups.sort_by(|left, right| {
        left.label_name
            .to_lowercase()
            .cmp(&right.label_name.to_lowercase())
            .then_with(|| left.label_name.cmp(&right.label_name))
    });
    groups
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

fn push_prefixed_text_block(lines: &mut Vec<Line<'static>>, text: &str, prefix: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        lines.push(Line::from(format!("{prefix}<empty>")));
        return;
    }

    for line in trimmed.lines() {
        lines.push(Line::from(format!("{prefix}{line}")));
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

fn panel_block(title: &str, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(if focused { Color::Cyan } else { Color::White })
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(border_style)
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
        List::new(debug_items).block(panel_block("websocket debug log • press D to close", false)),
        popup,
    );
}

fn draw_filter_editor_overlay(frame: &mut ratatui::Frame<'_>, area: Rect, app: &AppState) {
    let Some(editor) = app.filter_editor.as_ref() else {
        return;
    };

    let popup = centered_rect(72, 22, area);
    let chars = editor.buffer.chars().collect::<Vec<_>>();
    let cursor = editor.cursor.min(chars.len());
    let before = chars[..cursor].iter().collect::<String>();
    let at = chars
        .get(cursor)
        .map_or(" ".to_string(), ToString::to_string);
    let after = if cursor < chars.len() {
        chars[cursor + 1..].iter().collect::<String>()
    } else {
        String::new()
    };

    let lines = vec![
        Line::from(Span::styled(
            "Filter explorer",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(dim_span(
            "Live client-side filter • substring or glob (*, ?)",
        )),
        Line::from(dim_span("Enter keep • Esc clear/close • Ctrl-c force quit")),
        Line::from(""),
        Line::from(vec![
            Span::styled(before, Style::default().fg(Color::White)),
            Span::styled(
                at,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(after, Style::default().fg(Color::White)),
        ]),
    ];

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block("filter", true))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn draw_title_editor_overlay(frame: &mut ratatui::Frame<'_>, area: Rect, app: &AppState) {
    let Some(editor) = app.title_editor.as_ref() else {
        return;
    };

    let popup = centered_rect(72, 22, area);
    let chars = editor.buffer.chars().collect::<Vec<_>>();
    let cursor = editor.cursor.min(chars.len());
    let before = chars[..cursor].iter().collect::<String>();
    let at = chars
        .get(cursor)
        .map_or(" ".to_string(), ToString::to_string);
    let after = if cursor < chars.len() {
        chars[cursor + 1..].iter().collect::<String>()
    } else {
        String::new()
    };

    let lines = vec![
        Line::from(Span::styled(
            "Edit card title",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(dim_span("Enter save • Esc cancel • Ctrl-c force quit")),
        Line::from(""),
        Line::from(vec![
            Span::styled(before, Style::default().fg(Color::White)),
            Span::styled(
                at,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(after, Style::default().fg(Color::White)),
        ]),
    ];

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block("title editor", true))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn draw_saving_overlay(frame: &mut ratatui::Frame<'_>, area: Rect, app: &AppState) {
    let Some(card_id) = app.saving_card.as_deref() else {
        return;
    };

    let popup = centered_rect(58, 20, area);
    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 120;
    let spinner = spinner_frames[(tick as usize) % spinner_frames.len()];
    let label = app.saving_card_label().unwrap_or("selected card");

    let lines = vec![
        Line::from(vec![Span::styled(
            format!("{spinner} Saving card changes"),
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("card: ", Style::default().fg(Color::Cyan)),
            Span::raw(truncate(label, 54)),
        ]),
        Line::from(vec![
            Span::styled("id: ", Style::default().fg(Color::DarkGray)),
            Span::styled(truncate(card_id, 54), Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(dim_span(
            "Waiting for server response. Controls are paused until save succeeds or fails.",
        )),
        Line::from(dim_span(
            "Dialog stays visible for a short minimum time so fast saves are still readable.",
        )),
        Line::from(dim_span("Ctrl-c force quits if you get stuck.")),
    ];

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block("saving", true))
            .wrap(Wrap { trim: true }),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_high_bits() {
        assert_eq!(base64_encode(&[0xff, 0xff, 0xff]), "////");
        assert_eq!(base64_encode(&[0x00, 0x00, 0x00]), "AAAA");
    }

    fn fixture_card() -> CardSummary {
        CardSummary {
            id: "card-1".into(),
            name: "Fast COPY".into(),
            description: None,
            position: 0.0,
            is_closed: false,
            comments_total: 0,
            due_date: None,
            creator: None,
            labels: vec![],
            assignees: vec![],
            attachments: vec![],
            task_lists: vec![],
            is_subscribed: false,
        }
    }

    fn fixture_list(cards: Vec<CardSummary>) -> ListSummary {
        ListSummary {
            id: "list-1".into(),
            name: "Backlog".into(),
            position: 0.0,
            card_count: cards.len(),
            active_card_count: cards.len(),
            closed_card_count: 0,
            cards,
        }
    }

    fn fixture_board(lists: Vec<ListSummary>) -> BoardSummary {
        BoardSummary {
            id: "board-1".into(),
            name: "Work".into(),
            project_id: "proj-1".into(),
            position: 0.0,
            total_cards: 0,
            active_card_count: 0,
            closed_card_count: 0,
            active_lists: lists,
            labels: vec![],
            members: vec![],
        }
    }

    fn fixture_project(boards: Vec<BoardSummary>) -> ProjectTree {
        ProjectTree {
            id: "proj-1".into(),
            name: "planka-cli".into(),
            description: None,
            boards,
        }
    }

    fn fixture_tree() -> Vec<ProjectTree> {
        vec![fixture_project(vec![fixture_board(vec![fixture_list(
            vec![fixture_card()],
        )])])]
    }

    #[test]
    fn fast_copy_card() {
        let projects = fixture_tree();
        let payload = fast_copy_for(&projects, &TreeKey::Card("card-1".into())).unwrap();
        assert_eq!(
            payload.breadcrumb,
            "planka-cli > Work > Backlog > Fast COPY"
        );
        assert_eq!(
            payload.json,
            r#"{"project":{"id":"proj-1","name":"planka-cli"},"board":{"id":"board-1","name":"Work"},"list":{"id":"list-1","name":"Backlog"},"card":{"id":"card-1","name":"Fast COPY"}}"#
        );
        assert_eq!(
            payload.command,
            "# planka-cli > Work > Backlog > Fast COPY\nplnk card snapshot card-1 --output json\n"
        );
    }

    #[test]
    fn fast_copy_list() {
        let projects = fixture_tree();
        let payload = fast_copy_for(&projects, &TreeKey::List("list-1".into())).unwrap();
        assert_eq!(payload.breadcrumb, "planka-cli > Work > Backlog");
        assert_eq!(
            payload.json,
            r#"{"project":{"id":"proj-1","name":"planka-cli"},"board":{"id":"board-1","name":"Work"},"list":{"id":"list-1","name":"Backlog"}}"#
        );
        assert_eq!(
            payload.command,
            "# planka-cli > Work > Backlog\nplnk list get list-1 --output json\n"
        );
    }

    #[test]
    fn fast_copy_board() {
        let projects = fixture_tree();
        let payload = fast_copy_for(&projects, &TreeKey::Board("board-1".into())).unwrap();
        assert_eq!(payload.breadcrumb, "planka-cli > Work");
        assert_eq!(
            payload.command,
            "# planka-cli > Work\nplnk board snapshot board-1 --output json\n"
        );
    }

    #[test]
    fn fast_copy_project() {
        let projects = fixture_tree();
        let payload = fast_copy_for(&projects, &TreeKey::Project("proj-1".into())).unwrap();
        assert_eq!(payload.breadcrumb, "planka-cli");
        assert_eq!(
            payload.command,
            "# planka-cli\nplnk project snapshot proj-1 --output json\n"
        );
    }

    #[test]
    fn fast_copy_grouped_card_resolves() {
        let projects = fixture_tree();
        let payload = fast_copy_for(
            &projects,
            &TreeKey::GroupedCard {
                group_key: "any".into(),
                card_id: "card-1".into(),
            },
        )
        .unwrap();
        assert_eq!(
            payload.breadcrumb,
            "planka-cli > Work > Backlog > Fast COPY"
        );
    }

    #[test]
    fn fast_copy_label_group_resolves_to_list() {
        let projects = fixture_tree();
        let payload = fast_copy_for(
            &projects,
            &TreeKey::LabelGroup {
                board_id: "board-1".into(),
                list_id: "list-1".into(),
                label_id: None,
            },
        )
        .unwrap();
        assert_eq!(payload.breadcrumb, "planka-cli > Work > Backlog");
    }

    #[test]
    fn fast_copy_unknown_returns_none() {
        let projects = fixture_tree();
        assert!(fast_copy_for(&projects, &TreeKey::Card("missing".into())).is_none());
    }

    #[test]
    fn fast_copy_escapes_special_chars_in_json() {
        let mut projects = fixture_tree();
        projects[0].boards[0].active_lists[0].cards[0].name = "weird \"name\" \\ \n".into();
        let payload = fast_copy_for(&projects, &TreeKey::Card("card-1".into())).unwrap();
        assert!(payload.json.contains(r#""name":"weird \"name\" \\ \n""#));
    }

    #[test]
    fn fast_copy_command_blocks_newline_breakout_in_name() {
        let mut projects = fixture_tree();
        projects[0].boards[0].active_lists[0].cards[0].name = "evil\nrm -rf ~".into();
        let payload = fast_copy_for(&projects, &TreeKey::Card("card-1".into())).unwrap();

        let lines: Vec<&str> = payload.command.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "exactly 2 lines (1 comment + 1 plnk command); attacker name produced extra line: {:?}",
            payload.command
        );
        assert!(lines[0].starts_with("# "));
        assert!(lines[0].contains("evil rm -rf ~"));
        assert!(lines[1].starts_with("plnk card snapshot"));
    }

    #[test]
    fn fast_copy_command_strips_cr_and_escape_sequences() {
        let mut projects = fixture_tree();
        projects[0].boards[0].active_lists[0].cards[0].name = "a\rb\x1b[2Jc".into();
        let payload = fast_copy_for(&projects, &TreeKey::Card("card-1".into())).unwrap();

        let comment_line = payload.command.lines().next().unwrap();
        assert!(!comment_line.contains('\r'));
        assert!(!comment_line.contains('\x1b'));
        assert!(!comment_line.contains('\x00'));
    }

    #[test]
    fn fast_copy_command_replaces_controls_with_space() {
        let mut projects = fixture_tree();
        projects[0].boards[0].active_lists[0].cards[0].name = "a\nb".into();
        let payload = fast_copy_for(&projects, &TreeKey::Card("card-1".into())).unwrap();
        assert!(
            payload.command.contains("> a b\n"),
            "expected control char replaced with space, got: {:?}",
            payload.command
        );
    }
}
