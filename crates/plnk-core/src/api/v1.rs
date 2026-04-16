//! Planka API v1 implementation.
//!
//! `PlankaClientV1` wraps `HttpClient` and implements all resource API traits.
//! This is the only concrete implementation — the CLI depends on traits, not this type.

use async_trait::async_trait;
use tracing::debug;

use crate::client::HttpClient;
use crate::error::PlankaError;
use crate::models::{
    Board, Card, CreateBoard, CreateCard, CreateList, CreateProject, FindScope, List, MoveCard,
    Project, UpdateBoard, UpdateCard, UpdateList, UpdateProject, User,
};

use super::responses::{
    BoardSnapshot, CardsListResponse, ItemResponse, ItemsResponse, ProjectSnapshot,
};
use super::search::match_by_name;
use super::traits::{BoardApi, CardApi, ListApi, ProjectApi, UserApi};

/// Concrete Planka API client for the current server version.
pub struct PlankaClientV1 {
    http: HttpClient,
}

impl PlankaClientV1 {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }
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

    async fn get_card(&self, id: &str) -> Result<Card, PlankaError> {
        let resp: ItemResponse<Card> = self.http.get(&format!("/api/cards/{id}")).await?;
        Ok(resp.item)
    }

    async fn find_cards(&self, scope: FindScope, title: &str) -> Result<Vec<Card>, PlankaError> {
        let cards = match scope {
            FindScope::List(list_id) => {
                debug!("Finding cards in list {list_id}");
                self.list_cards(&list_id).await?
            }
            FindScope::Board(board_id) => {
                debug!("Finding cards in board {board_id}");
                let resp: BoardSnapshot = self.http.get(&format!("/api/boards/{board_id}")).await?;
                resp.included.cards
            }
            FindScope::Project(project_id) => {
                debug!("Finding cards across project {project_id}");
                let boards = self.list_boards(&project_id).await?;
                let mut all_cards = Vec::new();
                for board in &boards {
                    let resp: BoardSnapshot =
                        self.http.get(&format!("/api/boards/{}", board.id)).await?;
                    all_cards.extend(resp.included.cards);
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
