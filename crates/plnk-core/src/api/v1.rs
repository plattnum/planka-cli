//! Planka API v1 implementation.
//!
//! `PlankaClientV1` wraps `HttpClient` and implements all resource API traits.
//! This is the only concrete implementation — the CLI depends on traits, not this type.

use std::collections::HashSet;
use std::path::Path;

use async_trait::async_trait;
use tokio::task::JoinSet;
use tracing::debug;

use crate::client::HttpClient;
use crate::error::PlankaError;
use crate::models::{
    Attachment, Board, BoardMembership, Card, CardBatchFailure, CardBatchGetResult, CardLabel,
    CardMembership, Comment, CreateBoard, CreateCard, CreateCardMembership, CreateComment,
    CreateList, CreateProject, CreateTaskList, FindScope, Label, List, MoveCard, Project,
    ProjectManager, Task, UpdateBoard, UpdateCard, UpdateComment, UpdateLabel, UpdateList,
    UpdateProject, UpdateTask, User,
};

use super::responses::{
    BoardSnapshot, CardSnapshot, CardsListResponse, CommentsListResponse, ItemResponse,
    ItemsResponse, ProjectSnapshot,
};
use super::search::match_by_name;
use super::traits::{
    AssigneeApi, AttachmentApi, BoardApi, CardApi, CardLabelApi, CommentApi, LabelApi, ListApi,
    MembershipApi, ProjectApi, TaskApi, UserApi,
};

/// Concrete Planka API client for the current server version.
#[derive(Clone)]
pub struct PlankaClientV1 {
    http: HttpClient,
}

impl PlankaClientV1 {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }
}

fn filter_cards_from_board_snapshot(
    cards: &[Card],
    card_labels: &[CardLabel],
    list_id: Option<&str>,
    label_ids: &[String],
) -> Vec<Card> {
    let allowed_card_ids = if label_ids.is_empty() {
        None
    } else {
        let mut allowed: Option<HashSet<&str>> = None;

        for label_id in label_ids {
            let matching: HashSet<&str> = card_labels
                .iter()
                .filter(|card_label| card_label.label_id == *label_id)
                .map(|card_label| card_label.card_id.as_str())
                .collect();

            allowed = Some(match allowed {
                Some(existing) => existing.intersection(&matching).copied().collect(),
                None => matching,
            });
        }

        allowed
    };

    cards
        .iter()
        .filter(|card| {
            if let Some(target_list_id) = list_id
                && card.list_id != target_list_id
            {
                return false;
            }

            if let Some(allowed) = &allowed_card_ids {
                allowed.contains(card.id.as_str())
            } else {
                true
            }
        })
        .cloned()
        .collect()
}

// ── UserApi ──────────────────────────────────────────────────────────────

#[async_trait]
impl UserApi for PlankaClientV1 {
    async fn list_users(&self) -> Result<Vec<User>, PlankaError> {
        let resp: ItemsResponse<User> = self.http.get("/api/users").await?;
        Ok(resp.items)
    }

    async fn get_user(&self, id: &str) -> Result<User, PlankaError> {
        let resp: ItemResponse<User> = self.http.get(&format!("/api/users/{id}")).await?;
        Ok(resp.item)
    }
}

// ── ProjectApi ───────────────────────────────────────────────────────────

#[async_trait]
impl ProjectApi for PlankaClientV1 {
    async fn list_projects(&self) -> Result<Vec<Project>, PlankaError> {
        let resp: ItemsResponse<Project> = self.http.get("/api/projects").await?;
        Ok(resp.items)
    }

    async fn get_project(&self, id: &str) -> Result<Project, PlankaError> {
        let resp: ProjectSnapshot = self.http.get(&format!("/api/projects/{id}")).await?;
        Ok(resp.item)
    }

    async fn find_projects(&self, name: &str) -> Result<Vec<Project>, PlankaError> {
        let projects = self.list_projects().await?;
        let matched = match_by_name(&projects, name);
        Ok(matched.into_iter().cloned().collect())
    }

    async fn get_project_snapshot(&self, id: &str) -> Result<serde_json::Value, PlankaError> {
        self.http.get(&format!("/api/projects/{id}")).await
    }

    async fn create_project(&self, params: CreateProject) -> Result<Project, PlankaError> {
        let resp: ItemResponse<Project> = self.http.post("/api/projects", &params).await?;
        Ok(resp.item)
    }

    async fn update_project(
        &self,
        id: &str,
        params: UpdateProject,
    ) -> Result<Project, PlankaError> {
        let resp: ItemResponse<Project> = self
            .http
            .patch(&format!("/api/projects/{id}"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn delete_project(&self, id: &str) -> Result<(), PlankaError> {
        self.http.delete(&format!("/api/projects/{id}")).await
    }
}

// ── BoardApi ─────────────────────────────────────────────────────────────

#[async_trait]
impl BoardApi for PlankaClientV1 {
    async fn list_boards(&self, project_id: &str) -> Result<Vec<Board>, PlankaError> {
        // Boards are nested inside the project snapshot
        let resp: ProjectSnapshot = self
            .http
            .get(&format!("/api/projects/{project_id}"))
            .await?;
        Ok(resp.included.boards)
    }

    async fn get_board(&self, id: &str) -> Result<Board, PlankaError> {
        let resp: BoardSnapshot = self.http.get(&format!("/api/boards/{id}")).await?;
        Ok(Board {
            id: resp.item.id,
            project_id: resp.item.project_id,
            name: resp.item.name,
            position: resp.item.position,
            created_at: resp.item.created_at,
            updated_at: resp.item.updated_at,
        })
    }

    async fn find_boards(&self, project_id: &str, name: &str) -> Result<Vec<Board>, PlankaError> {
        let boards = self.list_boards(project_id).await?;
        let matched = match_by_name(&boards, name);
        Ok(matched.into_iter().cloned().collect())
    }

    async fn get_board_snapshot(&self, id: &str) -> Result<serde_json::Value, PlankaError> {
        self.http.get(&format!("/api/boards/{id}")).await
    }

    async fn create_board(
        &self,
        project_id: &str,
        params: CreateBoard,
    ) -> Result<Board, PlankaError> {
        let resp: ItemResponse<Board> = self
            .http
            .post(&format!("/api/projects/{project_id}/boards"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn update_board(&self, id: &str, params: UpdateBoard) -> Result<Board, PlankaError> {
        let resp: ItemResponse<Board> = self
            .http
            .patch(&format!("/api/boards/{id}"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn delete_board(&self, id: &str) -> Result<(), PlankaError> {
        self.http.delete(&format!("/api/boards/{id}")).await
    }
}

// ── ListApi ──────────────────────────────────────────────────────────────

#[async_trait]
impl ListApi for PlankaClientV1 {
    async fn list_lists(&self, board_id: &str) -> Result<Vec<List>, PlankaError> {
        // Lists are nested inside the board snapshot
        let resp: BoardSnapshot = self.http.get(&format!("/api/boards/{board_id}")).await?;
        // Filter to only "active" type lists (exclude archive lists with null name/position)
        let lists = resp
            .included
            .lists
            .into_iter()
            .filter(|l| !l.name.is_empty())
            .collect();
        Ok(lists)
    }

    async fn get_list(&self, id: &str) -> Result<List, PlankaError> {
        // No direct /api/lists/{id} endpoint — but PATCH works, so try GET
        // Planka doesn't have a direct list GET — we need to find it from the board.
        // However, we can use the PATCH endpoint pattern. Let's try the direct endpoint first.
        let resp: ItemResponse<List> = self.http.get(&format!("/api/lists/{id}")).await?;
        Ok(resp.item)
    }

    async fn find_lists(&self, board_id: &str, name: &str) -> Result<Vec<List>, PlankaError> {
        let lists = self.list_lists(board_id).await?;
        let matched = match_by_name(&lists, name);
        Ok(matched.into_iter().cloned().collect())
    }

    async fn create_list(&self, board_id: &str, params: CreateList) -> Result<List, PlankaError> {
        let resp: ItemResponse<List> = self
            .http
            .post(&format!("/api/boards/{board_id}/lists"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn update_list(&self, id: &str, params: UpdateList) -> Result<List, PlankaError> {
        let resp: ItemResponse<List> = self
            .http
            .patch(&format!("/api/lists/{id}"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn delete_list(&self, id: &str) -> Result<(), PlankaError> {
        self.http.delete(&format!("/api/lists/{id}")).await
    }
}

// ── CardApi ──────────────────────────────────────────────────────────────

#[async_trait]
impl CardApi for PlankaClientV1 {
    async fn list_cards(&self, list_id: &str) -> Result<Vec<Card>, PlankaError> {
        let resp: CardsListResponse = self
            .http
            .get(&format!("/api/lists/{list_id}/cards"))
            .await?;
        Ok(resp.items)
    }

    async fn list_cards_in_board(
        &self,
        board_id: &str,
        list_id: Option<&str>,
        label_ids: &[String],
    ) -> Result<Vec<Card>, PlankaError> {
        let resp: BoardSnapshot = self.http.get(&format!("/api/boards/{board_id}")).await?;
        Ok(filter_cards_from_board_snapshot(
            &resp.included.cards,
            &resp.included.card_labels,
            list_id,
            label_ids,
        ))
    }

    async fn get_card(&self, id: &str) -> Result<Card, PlankaError> {
        let resp: ItemResponse<Card> = self.http.get(&format!("/api/cards/{id}")).await?;
        Ok(resp.item)
    }

    async fn get_many_cards(
        &self,
        ids: Vec<String>,
        concurrency: usize,
    ) -> Result<CardBatchGetResult, PlankaError> {
        if concurrency == 0 {
            return Err(PlankaError::InvalidOptionValue {
                field: "concurrency".to_string(),
                message: "must be at least 1".to_string(),
            });
        }

        let requested_count = ids.len();
        if requested_count == 0 {
            return Ok(CardBatchGetResult {
                cards: Vec::new(),
                missing_ids: Vec::new(),
                failures: Vec::new(),
                requested_count: 0,
                concurrency: 0,
            });
        }

        let effective_concurrency = concurrency.min(self.http.transport_policy().max_in_flight);
        let mut pending = ids.into_iter().enumerate();
        let mut join_set = JoinSet::new();
        let mut found = Vec::new();
        let mut missing = Vec::new();
        let mut failures = Vec::new();

        let spawn_request = |join_set: &mut JoinSet<(usize, String, Result<Card, PlankaError>)>,
                             client: Self,
                             index: usize,
                             id: String| {
            join_set.spawn(async move {
                let result = client.get_card(&id).await;
                (index, id, result)
            });
        };

        for _ in 0..effective_concurrency {
            if let Some((index, id)) = pending.next() {
                spawn_request(&mut join_set, self.clone(), index, id);
            }
        }

        while let Some(next) = join_set.join_next().await {
            let (index, id, result) = next.map_err(|error| PlankaError::ApiError {
                status: 0,
                message: format!("card get-many worker failed: {error}"),
            })?;

            match result {
                Ok(card) => found.push((index, card)),
                Err(
                    error @ (PlankaError::Remote404 { .. }
                    | PlankaError::NotFound { .. }
                    | PlankaError::NotFoundMessage { .. }),
                ) => {
                    let _ = error;
                    missing.push((index, id));
                }
                Err(error) => failures.push((
                    index,
                    CardBatchFailure {
                        id,
                        error_type: error.error_type().to_string(),
                        message: error.to_string(),
                    },
                )),
            }

            if let Some((next_index, next_id)) = pending.next() {
                spawn_request(&mut join_set, self.clone(), next_index, next_id);
            }
        }

        found.sort_by_key(|(index, _)| *index);
        missing.sort_by_key(|(index, _)| *index);
        failures.sort_by_key(|(index, _)| *index);

        Ok(CardBatchGetResult {
            cards: found.into_iter().map(|(_, card)| card).collect(),
            missing_ids: missing.into_iter().map(|(_, id)| id).collect(),
            failures: failures.into_iter().map(|(_, failure)| failure).collect(),
            requested_count,
            concurrency: effective_concurrency,
        })
    }

    async fn get_card_snapshot(&self, id: &str) -> Result<serde_json::Value, PlankaError> {
        self.http.get(&format!("/api/cards/{id}")).await
    }

    async fn find_cards(&self, scope: FindScope, title: &str) -> Result<Vec<Card>, PlankaError> {
        let cards = match scope {
            FindScope::List(list_id) => {
                debug!("Finding cards in list {list_id}");
                self.list_cards(&list_id).await?
            }
            FindScope::Board(board_id) => {
                debug!("Finding cards in board {board_id}");
                self.list_cards_in_board(&board_id, None, &[]).await?
            }
            FindScope::Project(project_id) => {
                debug!("Finding cards across project {project_id}");
                let boards = self.list_boards(&project_id).await?;
                let mut all_cards = Vec::new();
                for board in &boards {
                    all_cards.extend(self.list_cards_in_board(&board.id, None, &[]).await?);
                }
                all_cards
            }
        };
        let matched = match_by_name(&cards, title);
        Ok(matched.into_iter().cloned().collect())
    }

    async fn create_card(&self, list_id: &str, params: CreateCard) -> Result<Card, PlankaError> {
        let resp: ItemResponse<Card> = self
            .http
            .post(&format!("/api/lists/{list_id}/cards"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn update_card(&self, id: &str, params: UpdateCard) -> Result<Card, PlankaError> {
        let resp: ItemResponse<Card> = self
            .http
            .patch(&format!("/api/cards/{id}"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn move_card(&self, id: &str, params: MoveCard) -> Result<Card, PlankaError> {
        // Move is implemented as a PATCH with listId and position
        let resp: ItemResponse<Card> = self
            .http
            .patch(&format!("/api/cards/{id}"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn delete_card(&self, id: &str) -> Result<(), PlankaError> {
        self.http.delete(&format!("/api/cards/{id}")).await
    }

    async fn archive_card(&self, id: &str) -> Result<Card, PlankaError> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct ArchivePayload {
            is_closed: bool,
        }

        let resp: ItemResponse<Card> = self
            .http
            .patch(
                &format!("/api/cards/{id}"),
                &ArchivePayload { is_closed: true },
            )
            .await?;
        Ok(resp.item)
    }

    async fn unarchive_card(&self, id: &str) -> Result<Card, PlankaError> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct ArchivePayload {
            is_closed: bool,
        }

        let resp: ItemResponse<Card> = self
            .http
            .patch(
                &format!("/api/cards/{id}"),
                &ArchivePayload { is_closed: false },
            )
            .await?;
        Ok(resp.item)
    }
}

// ── TaskApi ─────────────────────────────────────────────────────────────

#[async_trait]
impl TaskApi for PlankaClientV1 {
    async fn list_tasks(&self, card_id: &str) -> Result<Vec<Task>, PlankaError> {
        // Tasks come from card's included data
        let resp: CardSnapshot = self.http.get(&format!("/api/cards/{card_id}")).await?;
        Ok(resp.included.tasks)
    }

    async fn create_task(&self, card_id: &str, name: &str) -> Result<Task, PlankaError> {
        // Tasks live inside task lists. Find or create a default task list.
        let resp: CardSnapshot = self.http.get(&format!("/api/cards/{card_id}")).await?;
        let task_list_id = if let Some(tl) = resp.included.task_lists.first() {
            tl.id.clone()
        } else {
            debug!("No task list found, creating default");
            let params = CreateTaskList {
                name: "Tasks".to_string(),
                position: 65536.0,
            };
            let tl_resp: ItemResponse<crate::models::TaskList> = self
                .http
                .post(&format!("/api/cards/{card_id}/task-lists"), &params)
                .await?;
            tl_resp.item.id
        };

        let params = crate::models::CreateTask {
            name: name.to_string(),
            position: 65536.0,
        };
        let resp: ItemResponse<Task> = self
            .http
            .post(&format!("/api/task-lists/{task_list_id}/tasks"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn update_task(&self, id: &str, params: UpdateTask) -> Result<Task, PlankaError> {
        let resp: ItemResponse<Task> = self
            .http
            .patch(&format!("/api/tasks/{id}"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn complete_task(&self, id: &str) -> Result<Task, PlankaError> {
        self.update_task(
            id,
            UpdateTask {
                name: None,
                is_completed: Some(true),
            },
        )
        .await
    }

    async fn reopen_task(&self, id: &str) -> Result<Task, PlankaError> {
        self.update_task(
            id,
            UpdateTask {
                name: None,
                is_completed: Some(false),
            },
        )
        .await
    }

    async fn delete_task(&self, id: &str) -> Result<(), PlankaError> {
        self.http.delete(&format!("/api/tasks/{id}")).await
    }
}

// ── CommentApi ──────────────────────────────────────────────────────────

#[async_trait]
impl CommentApi for PlankaClientV1 {
    async fn list_comments(&self, card_id: &str) -> Result<Vec<Comment>, PlankaError> {
        let resp: CommentsListResponse = self
            .http
            .get(&format!("/api/cards/{card_id}/comments"))
            .await?;
        Ok(resp.items)
    }

    async fn create_comment(
        &self,
        card_id: &str,
        params: CreateComment,
    ) -> Result<Comment, PlankaError> {
        let resp: ItemResponse<Comment> = self
            .http
            .post(&format!("/api/cards/{card_id}/comments"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn update_comment(
        &self,
        id: &str,
        params: UpdateComment,
    ) -> Result<Comment, PlankaError> {
        let resp: ItemResponse<Comment> = self
            .http
            .patch(&format!("/api/comments/{id}"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn delete_comment(&self, id: &str) -> Result<(), PlankaError> {
        self.http.delete(&format!("/api/comments/{id}")).await
    }
}

// ── LabelApi ────────────────────────────────────────────────────────────

#[async_trait]
impl LabelApi for PlankaClientV1 {
    async fn list_labels(&self, board_id: &str) -> Result<Vec<Label>, PlankaError> {
        // Labels come from board snapshot's included data
        let resp: BoardSnapshot = self.http.get(&format!("/api/boards/{board_id}")).await?;
        Ok(resp.included.labels)
    }

    async fn find_labels(&self, board_id: &str, name: &str) -> Result<Vec<Label>, PlankaError> {
        let labels = self.list_labels(board_id).await?;
        let matched = match_by_name(&labels, name);
        Ok(matched.into_iter().cloned().collect())
    }

    async fn create_label(
        &self,
        board_id: &str,
        name: &str,
        color: &str,
    ) -> Result<Label, PlankaError> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CreateLabelPayload {
            name: String,
            color: String,
            position: f64,
        }

        let payload = CreateLabelPayload {
            name: name.to_string(),
            color: color.to_string(),
            position: 65536.0,
        };
        let resp: ItemResponse<Label> = self
            .http
            .post(&format!("/api/boards/{board_id}/labels"), &payload)
            .await?;
        Ok(resp.item)
    }

    async fn update_label(&self, id: &str, params: UpdateLabel) -> Result<Label, PlankaError> {
        let resp: ItemResponse<Label> = self
            .http
            .patch(&format!("/api/labels/{id}"), &params)
            .await?;
        Ok(resp.item)
    }

    async fn delete_label(&self, id: &str) -> Result<(), PlankaError> {
        self.http.delete(&format!("/api/labels/{id}")).await
    }
}

// ── CardLabelApi ────────────────────────────────────────────────────────

#[async_trait]
impl CardLabelApi for PlankaClientV1 {
    async fn list_card_labels(&self, card_id: &str) -> Result<Vec<CardLabel>, PlankaError> {
        let resp: CardSnapshot = self.http.get(&format!("/api/cards/{card_id}")).await?;
        Ok(resp.included.card_labels)
    }

    async fn add_card_label(
        &self,
        card_id: &str,
        label_id: &str,
    ) -> Result<CardLabel, PlankaError> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct AddLabelPayload {
            label_id: String,
        }

        let payload = AddLabelPayload {
            label_id: label_id.to_string(),
        };
        let resp: ItemResponse<CardLabel> = self
            .http
            .post(&format!("/api/cards/{card_id}/card-labels"), &payload)
            .await?;
        Ok(resp.item)
    }

    async fn remove_card_label(&self, card_id: &str, label_id: &str) -> Result<(), PlankaError> {
        self.http
            .delete(&format!(
                "/api/cards/{card_id}/card-labels/labelId:{label_id}"
            ))
            .await
    }
}

// ── AssigneeApi ─────────────────────────────────────────────────────────

#[async_trait]
impl AssigneeApi for PlankaClientV1 {
    async fn list_assignees(&self, card_id: &str) -> Result<Vec<CardMembership>, PlankaError> {
        let resp: CardSnapshot = self.http.get(&format!("/api/cards/{card_id}")).await?;
        Ok(resp.included.card_memberships)
    }

    async fn add_assignee(
        &self,
        card_id: &str,
        user_id: &str,
    ) -> Result<CardMembership, PlankaError> {
        let payload = CreateCardMembership {
            user_id: user_id.to_string(),
        };
        let resp: ItemResponse<CardMembership> = self
            .http
            .post(&format!("/api/cards/{card_id}/card-memberships"), &payload)
            .await?;
        Ok(resp.item)
    }

    async fn remove_assignee(&self, card_id: &str, user_id: &str) -> Result<(), PlankaError> {
        self.http
            .delete(&format!(
                "/api/cards/{card_id}/card-memberships/userId:{user_id}"
            ))
            .await
    }
}

// ── AttachmentApi ───────────────────────────────────────────────────────

#[async_trait]
impl AttachmentApi for PlankaClientV1 {
    async fn list_attachments(&self, card_id: &str) -> Result<Vec<Attachment>, PlankaError> {
        let resp: CardSnapshot = self.http.get(&format!("/api/cards/{card_id}")).await?;
        Ok(resp.included.attachments)
    }

    async fn get_attachment(&self, id: &str) -> Result<Attachment, PlankaError> {
        // Attachments don't have direct GET. Get from card included data
        // would require card_id. For now, this is not supported by Planka API.
        Err(PlankaError::ApiError {
            status: 501,
            message: format!(
                "Direct attachment GET not supported by Planka API. Use 'attachment list --card <cardId>' to find attachment {id}."
            ),
        })
    }

    async fn upload_attachment(
        &self,
        card_id: &str,
        file_path: &Path,
    ) -> Result<Attachment, PlankaError> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("upload")
            .to_string();

        let file_bytes =
            tokio::fs::read(file_path)
                .await
                .map_err(|e| PlankaError::FileReadError {
                    path: file_path.display().to_string(),
                    source: e,
                })?;

        // Two quirks of Planka's Sails/skipper multipart body parser to match:
        //
        // 1. Text fields MUST precede the file field. skipper consumes text fields
        //    only up to the first file part, then treats the rest of the form as
        //    file data. If `type` and `name` arrive after `file`, they are silently
        //    dropped and the request fails with a 400 "missing/invalid parameters".
        //    (curl happens to send text-before-file by default, which is why curl
        //    uploads work while a naive reqwest upload does not.)
        //
        // 2. The file part must carry an explicit Content-Type. reqwest's
        //    Part::bytes() does NOT set one by default; we derive it from the
        //    file extension, falling back to application/octet-stream.
        let mime = mime_guess::from_path(file_path)
            .first_or_octet_stream()
            .essence_str()
            .to_string();
        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name.clone())
            .mime_str(&mime)
            .map_err(|e| PlankaError::ApiError {
                status: 0,
                message: format!("Invalid MIME type '{mime}': {e}"),
            })?;
        let form = reqwest::multipart::Form::new()
            .text("type", "file")
            .text("name", file_name)
            .part("file", part);

        let resp: ItemResponse<Attachment> = self
            .http
            .post_multipart(&format!("/api/cards/{card_id}/attachments"), form)
            .await?;
        Ok(resp.item)
    }

    async fn download_attachment(
        &self,
        card_id: &str,
        attachment_id: &str,
        out_path: Option<&Path>,
    ) -> Result<std::path::PathBuf, PlankaError> {
        // Fetch card snapshot to find the attachment metadata
        let resp: CardSnapshot = self.http.get(&format!("/api/cards/{card_id}")).await?;
        let attachment = resp
            .included
            .attachments
            .iter()
            .find(|a| a.id == attachment_id)
            .ok_or_else(|| PlankaError::NotFound {
                resource_type: "attachment".to_string(),
                id: attachment_id.to_string(),
            })?;

        // Build download path from attachment data.
        // data.url is absolute (http://host/attachments/id/download/name).
        // Extract the path portion starting from /attachments/.
        let download_path =
            if let Some(url) = attachment.data.as_ref().and_then(|d| d.url.as_deref()) {
                if let Some(idx) = url.find("/attachments/") {
                    url[idx..].to_string()
                } else {
                    url.to_string()
                }
            } else {
                format!("/attachments/{attachment_id}/download/{}", attachment.name)
            };

        let bytes = self.http.get_bytes(&download_path).await?;

        // Determine output path: --out if given, otherwise attachment's real name
        let final_path = match out_path {
            Some(p) => p.to_path_buf(),
            None => std::path::PathBuf::from(&attachment.name),
        };

        if let Some(parent) = final_path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        tokio::fs::write(&final_path, bytes).await?;
        Ok(final_path)
    }

    async fn delete_attachment(&self, id: &str) -> Result<(), PlankaError> {
        self.http.delete(&format!("/api/attachments/{id}")).await
    }
}

// ── MembershipApi ───────────────────────────────────────────────────────

#[async_trait]
impl MembershipApi for PlankaClientV1 {
    async fn list_board_members(
        &self,
        board_id: &str,
    ) -> Result<Vec<BoardMembership>, PlankaError> {
        let resp: BoardSnapshot = self.http.get(&format!("/api/boards/{board_id}")).await?;
        Ok(resp.included.board_memberships)
    }

    async fn list_project_managers(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectManager>, PlankaError> {
        let resp: ProjectSnapshot = self
            .http
            .get(&format!("/api/projects/{project_id}"))
            .await?;
        Ok(resp.included.project_managers)
    }

    async fn add_board_member(
        &self,
        board_id: &str,
        user_id: &str,
        role: Option<&str>,
    ) -> Result<BoardMembership, PlankaError> {
        let payload = crate::models::CreateBoardMembership {
            user_id: user_id.to_string(),
            role: role.map(String::from),
        };
        let resp: ItemResponse<BoardMembership> = self
            .http
            .post(
                &format!("/api/boards/{board_id}/board-memberships"),
                &payload,
            )
            .await?;
        Ok(resp.item)
    }

    async fn add_project_manager(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<ProjectManager, PlankaError> {
        let payload = crate::models::CreateProjectManager {
            user_id: user_id.to_string(),
        };
        let resp: ItemResponse<ProjectManager> = self
            .http
            .post(
                &format!("/api/projects/{project_id}/project-managers"),
                &payload,
            )
            .await?;
        Ok(resp.item)
    }

    async fn remove_board_member(&self, id: &str) -> Result<(), PlankaError> {
        self.http
            .delete(&format!("/api/board-memberships/{id}"))
            .await
    }

    async fn remove_project_manager(&self, id: &str) -> Result<(), PlankaError> {
        self.http
            .delete(&format!("/api/project-managers/{id}"))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::filter_cards_from_board_snapshot;
    use crate::models::{Card, CardLabel};

    fn card(id: &str, list_id: &str, name: &str) -> Card {
        Card {
            id: id.to_string(),
            list_id: list_id.to_string(),
            board_id: "board-1".to_string(),
            name: name.to_string(),
            description: None,
            position: 65_536.0,
            due_date: None,
            is_due_completed: None,
            is_closed: false,
            is_subscribed: false,
            creator_user_id: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: None,
        }
    }

    fn card_label(card_id: &str, label_id: &str) -> CardLabel {
        CardLabel {
            id: format!("{card_id}-{label_id}"),
            card_id: card_id.to_string(),
            label_id: label_id.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn snapshot_filter_by_single_label() {
        let cards = vec![
            card("card-1", "list-a", "One"),
            card("card-2", "list-a", "Two"),
            card("card-3", "list-b", "Three"),
        ];
        let card_labels = vec![
            card_label("card-1", "label-red"),
            card_label("card-2", "label-blue"),
            card_label("card-3", "label-red"),
        ];

        let filtered =
            filter_cards_from_board_snapshot(&cards, &card_labels, None, &["label-red".into()]);

        assert_eq!(
            filtered
                .iter()
                .map(|card| card.id.as_str())
                .collect::<Vec<_>>(),
            vec!["card-1", "card-3"]
        );
    }

    #[test]
    fn snapshot_filter_by_multiple_labels_uses_and_semantics() {
        let cards = vec![
            card("card-1", "list-a", "One"),
            card("card-2", "list-a", "Two"),
            card("card-3", "list-b", "Three"),
        ];
        let card_labels = vec![
            card_label("card-1", "label-red"),
            card_label("card-1", "label-blue"),
            card_label("card-2", "label-red"),
            card_label("card-3", "label-blue"),
        ];

        let filtered = filter_cards_from_board_snapshot(
            &cards,
            &card_labels,
            None,
            &["label-red".into(), "label-blue".into()],
        );

        assert_eq!(
            filtered
                .iter()
                .map(|card| card.id.as_str())
                .collect::<Vec<_>>(),
            vec!["card-1"]
        );
    }

    #[test]
    fn snapshot_filter_applies_list_scope_after_label_filter() {
        let cards = vec![
            card("card-1", "list-a", "One"),
            card("card-2", "list-a", "Two"),
            card("card-3", "list-b", "Three"),
        ];
        let card_labels = vec![
            card_label("card-1", "label-red"),
            card_label("card-3", "label-red"),
        ];

        let filtered = filter_cards_from_board_snapshot(
            &cards,
            &card_labels,
            Some("list-a"),
            &["label-red".into()],
        );

        assert_eq!(
            filtered
                .iter()
                .map(|card| card.id.as_str())
                .collect::<Vec<_>>(),
            vec!["card-1"]
        );
    }
}
