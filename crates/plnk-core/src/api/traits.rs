//! API capability traits for Planka resources.
//!
//! Each trait defines the operations available for a resource type.
//! The CLI depends on these traits, not on concrete implementations.
//! Today's implementation is `PlankaClientV1` in `v1.rs`.

use std::path::Path;

use async_trait::async_trait;

use crate::error::PlankaError;
use crate::models::{
    Attachment, Board, BoardMembership, Card, CardLabel, CardMembership, Comment, CreateBoard,
    CreateCard, CreateComment, CreateList, CreateProject, FindScope, Label, List, MoveCard,
    Project, ProjectManager, Task, UpdateBoard, UpdateCard, UpdateComment, UpdateLabel, UpdateList,
    UpdateProject, UpdateTask, User,
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

#[async_trait]
pub trait TaskApi {
    async fn list_tasks(&self, card_id: &str) -> Result<Vec<Task>, PlankaError>;
    async fn get_task(&self, id: &str) -> Result<Task, PlankaError>;
    async fn create_task(&self, card_id: &str, name: &str) -> Result<Task, PlankaError>;
    async fn update_task(&self, id: &str, params: UpdateTask) -> Result<Task, PlankaError>;
    async fn complete_task(&self, id: &str) -> Result<Task, PlankaError>;
    async fn reopen_task(&self, id: &str) -> Result<Task, PlankaError>;
    async fn delete_task(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait CommentApi {
    async fn list_comments(&self, card_id: &str) -> Result<Vec<Comment>, PlankaError>;
    async fn get_comment(&self, id: &str) -> Result<Comment, PlankaError>;
    async fn create_comment(
        &self,
        card_id: &str,
        params: CreateComment,
    ) -> Result<Comment, PlankaError>;
    async fn update_comment(&self, id: &str, params: UpdateComment)
    -> Result<Comment, PlankaError>;
    async fn delete_comment(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait LabelApi {
    async fn list_labels(&self, board_id: &str) -> Result<Vec<Label>, PlankaError>;
    async fn get_label(&self, id: &str) -> Result<Label, PlankaError>;
    async fn find_labels(&self, board_id: &str, name: &str) -> Result<Vec<Label>, PlankaError>;
    async fn create_label(
        &self,
        board_id: &str,
        name: &str,
        color: &str,
    ) -> Result<Label, PlankaError>;
    async fn update_label(&self, id: &str, params: UpdateLabel) -> Result<Label, PlankaError>;
    async fn delete_label(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait CardLabelApi {
    async fn list_card_labels(&self, card_id: &str) -> Result<Vec<CardLabel>, PlankaError>;
    async fn add_card_label(&self, card_id: &str, label_id: &str)
    -> Result<CardLabel, PlankaError>;
    async fn remove_card_label(&self, card_id: &str, label_id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait AssigneeApi {
    async fn list_assignees(&self, card_id: &str) -> Result<Vec<CardMembership>, PlankaError>;
    async fn add_assignee(
        &self,
        card_id: &str,
        user_id: &str,
    ) -> Result<CardMembership, PlankaError>;
    async fn remove_assignee(&self, card_id: &str, user_id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait AttachmentApi {
    async fn list_attachments(&self, card_id: &str) -> Result<Vec<Attachment>, PlankaError>;
    async fn get_attachment(&self, id: &str) -> Result<Attachment, PlankaError>;
    async fn upload_attachment(
        &self,
        card_id: &str,
        file_path: &Path,
    ) -> Result<Attachment, PlankaError>;
    async fn download_attachment(
        &self,
        card_id: &str,
        attachment_id: &str,
        out_path: Option<&Path>,
    ) -> Result<std::path::PathBuf, PlankaError>;
    async fn delete_attachment(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait MembershipApi {
    async fn list_board_members(&self, board_id: &str)
    -> Result<Vec<BoardMembership>, PlankaError>;
    async fn list_project_managers(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectManager>, PlankaError>;
    async fn add_board_member(
        &self,
        board_id: &str,
        user_id: &str,
        role: Option<&str>,
    ) -> Result<BoardMembership, PlankaError>;
    async fn add_project_manager(
        &self,
        project_id: &str,
        user_id: &str,
    ) -> Result<ProjectManager, PlankaError>;
    async fn remove_board_member(&self, id: &str) -> Result<(), PlankaError>;
    async fn remove_project_manager(&self, id: &str) -> Result<(), PlankaError>;
}
