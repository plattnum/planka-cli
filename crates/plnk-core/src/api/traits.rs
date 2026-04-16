//! API capability traits for Planka resources.
//!
//! Each trait defines the operations available for a resource type.
//! The CLI depends on these traits, not on concrete implementations.
//! Today's implementation is `PlankaClientV1` in `v1.rs`.

use async_trait::async_trait;

use crate::error::PlankaError;
use crate::models::{
    Board, Card, CreateBoard, CreateCard, CreateList, CreateProject, FindScope, List, MoveCard,
    Project, UpdateBoard, UpdateCard, UpdateList, UpdateProject, User,
};

#[async_trait]
pub trait UserApi {
    async fn list_users(&self) -> Result<Vec<User>, PlankaError>;
    async fn get_user(&self, id: &str) -> Result<User, PlankaError>;
}

#[async_trait]
pub trait ProjectApi {
    async fn list_projects(&self) -> Result<Vec<Project>, PlankaError>;
    async fn get_project(&self, id: &str) -> Result<Project, PlankaError>;
    async fn create_project(&self, params: CreateProject) -> Result<Project, PlankaError>;
    async fn update_project(&self, id: &str, params: UpdateProject)
    -> Result<Project, PlankaError>;
    async fn delete_project(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait BoardApi {
    async fn list_boards(&self, project_id: &str) -> Result<Vec<Board>, PlankaError>;
    async fn get_board(&self, id: &str) -> Result<Board, PlankaError>;
    async fn find_boards(&self, project_id: &str, name: &str) -> Result<Vec<Board>, PlankaError>;
    async fn create_board(
        &self,
        project_id: &str,
        params: CreateBoard,
    ) -> Result<Board, PlankaError>;
    async fn update_board(&self, id: &str, params: UpdateBoard) -> Result<Board, PlankaError>;
    async fn delete_board(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait ListApi {
    async fn list_lists(&self, board_id: &str) -> Result<Vec<List>, PlankaError>;
    async fn get_list(&self, id: &str) -> Result<List, PlankaError>;
    async fn find_lists(&self, board_id: &str, name: &str) -> Result<Vec<List>, PlankaError>;
    async fn create_list(&self, board_id: &str, params: CreateList) -> Result<List, PlankaError>;
    async fn update_list(&self, id: &str, params: UpdateList) -> Result<List, PlankaError>;
    async fn delete_list(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait CardApi {
    async fn list_cards(&self, list_id: &str) -> Result<Vec<Card>, PlankaError>;
    async fn get_card(&self, id: &str) -> Result<Card, PlankaError>;
    async fn find_cards(&self, scope: FindScope, title: &str) -> Result<Vec<Card>, PlankaError>;
    async fn create_card(&self, list_id: &str, params: CreateCard) -> Result<Card, PlankaError>;
    async fn update_card(&self, id: &str, params: UpdateCard) -> Result<Card, PlankaError>;
    async fn move_card(&self, id: &str, params: MoveCard) -> Result<Card, PlankaError>;
    async fn delete_card(&self, id: &str) -> Result<(), PlankaError>;
    async fn archive_card(&self, id: &str) -> Result<Card, PlankaError>;
    async fn unarchive_card(&self, id: &str) -> Result<Card, PlankaError>;
}
