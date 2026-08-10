//! API capability traits for Planka resources.
//!
//! Each trait defines the operations available for a resource type.
//! The CLI depends on these traits, not on concrete implementations.
//! Today's implementation is `PlankaClientV1` in `v1.rs`.

use std::path::Path;

use async_trait::async_trait;

use crate::error::PlankaError;
use crate::models::{
    Attachment, BaseCustomFieldGroup, Board, BoardMembership, Card, CardBatchGetResult, CardLabel,
    CardMembership, Comment, CreateBoard, CreateCard, CreateComment, CreateList, CreateProject,
    CustomField, CustomFieldGroup, CustomFieldValue, FindScope, Label, List, MoveCard, Project,
    ProjectManager, Task, UpdateBoard, UpdateCard, UpdateComment, UpdateCustomField,
    UpdateCustomFieldGroup, UpdateLabel, UpdateList, UpdateProject, UpdateTask, User,
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
    /// Return the full `GET /api/projects/{id}` response verbatim, including
    /// the `item` and every key under `included`. Kept as raw JSON so fields
    /// we don't formally model (custom fields, notification services, etc.)
    /// are preserved without schema loss.
    async fn get_project_snapshot(&self, id: &str) -> Result<serde_json::Value, PlankaError>;
    /// Find projects by name. Unscoped — projects are the root resource
    /// and have no parent to scope against. Uses three-tier name matching.
    async fn find_projects(&self, name: &str) -> Result<Vec<Project>, PlankaError>;
    async fn create_project(&self, params: CreateProject) -> Result<Project, PlankaError>;
    async fn update_project(&self, id: &str, params: UpdateProject)
    -> Result<Project, PlankaError>;
    async fn delete_project(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait BoardApi {
    async fn list_boards(&self, project_id: &str) -> Result<Vec<Board>, PlankaError>;
    async fn get_board(&self, id: &str) -> Result<Board, PlankaError>;
    /// Return the full `GET /api/boards/{id}` response verbatim. See
    /// `ProjectApi::get_project_snapshot` for rationale.
    async fn get_board_snapshot(&self, id: &str) -> Result<serde_json::Value, PlankaError>;
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
    /// Enumerate cards from a board snapshot, optionally narrowed to a single
    /// list and/or a set of labels that are AND-combined.
    async fn list_cards_in_board(
        &self,
        board_id: &str,
        list_id: Option<&str>,
        label_ids: &[String],
    ) -> Result<Vec<Card>, PlankaError>;
    async fn get_card(&self, id: &str) -> Result<Card, PlankaError>;
    async fn get_many_cards(
        &self,
        ids: Vec<String>,
        concurrency: usize,
    ) -> Result<CardBatchGetResult, PlankaError>;
    /// Return the full `GET /api/cards/{id}` response verbatim. See
    /// `ProjectApi::get_project_snapshot` for rationale.
    async fn get_card_snapshot(&self, id: &str) -> Result<serde_json::Value, PlankaError>;
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
    async fn create_task(&self, card_id: &str, name: &str) -> Result<Task, PlankaError>;
    async fn update_task(&self, id: &str, params: UpdateTask) -> Result<Task, PlankaError>;
    async fn complete_task(&self, id: &str) -> Result<Task, PlankaError>;
    async fn reopen_task(&self, id: &str) -> Result<Task, PlankaError>;
    async fn delete_task(&self, id: &str) -> Result<(), PlankaError>;
}

#[async_trait]
pub trait CommentApi {
    async fn list_comments(&self, card_id: &str) -> Result<Vec<Comment>, PlankaError>;
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

/// Custom field groups: the reusable project-level templates (base groups) and
/// the board- and card-level groups that hold fields.
///
/// Base groups are a separate resource from ordinary groups on the wire, with
/// their own routes, and are therefore given their own methods here rather than
/// being folded into the group methods. In particular there is **no
/// `GET /api/base-custom-field-groups/{id}`** route — that path falls through to
/// the Planka SPA and returns HTML with `200` — so `get_base_field_group` reads
/// the projects list instead.
#[async_trait]
pub trait CustomFieldGroupApi {
    /// Base groups defined on a project, read from the project snapshot.
    async fn list_base_field_groups(
        &self,
        project_id: &str,
    ) -> Result<Vec<BaseCustomFieldGroup>, PlankaError>;
    /// Look up a single base group by id.
    ///
    /// Reads `GET /api/projects` because no by-id route exists for base groups.
    async fn get_base_field_group(&self, id: &str) -> Result<BaseCustomFieldGroup, PlankaError>;
    async fn list_field_groups_for_board(
        &self,
        board_id: &str,
    ) -> Result<Vec<CustomFieldGroup>, PlankaError>;
    async fn list_field_groups_for_card(
        &self,
        card_id: &str,
    ) -> Result<Vec<CustomFieldGroup>, PlankaError>;
    async fn get_field_group(&self, id: &str) -> Result<CustomFieldGroup, PlankaError>;
    async fn create_base_field_group(
        &self,
        project_id: &str,
        name: &str,
    ) -> Result<BaseCustomFieldGroup, PlankaError>;
    async fn create_board_field_group(
        &self,
        board_id: &str,
        name: &str,
    ) -> Result<CustomFieldGroup, PlankaError>;
    /// Attach a group to a card, either by adopting a base group (`base_id`) or
    /// as a one-off named group. Exactly one of the two must be supplied.
    async fn create_card_field_group(
        &self,
        card_id: &str,
        base_id: Option<&str>,
        name: Option<&str>,
    ) -> Result<CustomFieldGroup, PlankaError>;
    async fn update_field_group(
        &self,
        id: &str,
        params: UpdateCustomFieldGroup,
    ) -> Result<CustomFieldGroup, PlankaError>;
    async fn update_base_field_group(
        &self,
        id: &str,
        params: UpdateCustomFieldGroup,
    ) -> Result<BaseCustomFieldGroup, PlankaError>;
    async fn delete_field_group(&self, id: &str) -> Result<(), PlankaError>;
    async fn delete_base_field_group(&self, id: &str) -> Result<(), PlankaError>;
}

/// Custom fields: the named slots inside a group.
///
/// Every method takes a `base` flag alongside the group id because base groups
/// and ordinary groups are distinct wire resources reached by different routes.
/// Reading a base group's fields is only possible through the projects list.
#[async_trait]
pub trait CustomFieldApi {
    async fn list_fields(
        &self,
        group_id: &str,
        base: bool,
    ) -> Result<Vec<CustomField>, PlankaError>;
    /// Every custom field defined by the card's own groups, read from the card
    /// snapshot in a single call.
    ///
    /// Fields belonging to an adopted group's *base* group are not included —
    /// those live on the base group and must be fetched with
    /// `list_fields(base_id, true)`.
    async fn list_fields_for_card(&self, card_id: &str) -> Result<Vec<CustomField>, PlankaError>;
    async fn find_fields(
        &self,
        group_id: &str,
        base: bool,
        name: &str,
    ) -> Result<Vec<CustomField>, PlankaError>;
    async fn create_field(
        &self,
        group_id: &str,
        base: bool,
        name: &str,
        show_on_front: bool,
    ) -> Result<CustomField, PlankaError>;
    async fn update_field(
        &self,
        id: &str,
        params: UpdateCustomField,
    ) -> Result<CustomField, PlankaError>;
    async fn delete_field(&self, id: &str) -> Result<(), PlankaError>;
}

/// Custom field values stored against a card.
#[async_trait]
pub trait CardCustomFieldApi {
    async fn list_field_values(&self, card_id: &str) -> Result<Vec<CustomFieldValue>, PlankaError>;
    async fn set_field_value(
        &self,
        card_id: &str,
        group_id: &str,
        field_id: &str,
        content: &str,
    ) -> Result<CustomFieldValue, PlankaError>;
    /// Remove a value. Idempotent: an already-unset value is a success.
    async fn clear_field_value(
        &self,
        card_id: &str,
        group_id: &str,
        field_id: &str,
    ) -> Result<(), PlankaError>;
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
