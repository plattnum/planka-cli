use plnk_core::api::BoardApi;
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateBoard, UpdateBoard};

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::output::{render_collection, render_item, render_message, render_snapshot};

pub async fn execute(
    client: &impl BoardApi,
    action: crate::app::BoardAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::BoardAction::List { project } => {
            let boards = client.list_boards(&project).await?;
            render_collection(&boards, format, full);
        }
        crate::app::BoardAction::Get { id } => {
            let board = client.get_board(&id).await?;
            render_item(&board, format, full);
        }
        crate::app::BoardAction::Snapshot { id } => {
            let snapshot = client.get_board_snapshot(&id).await?;
            render_snapshot(&snapshot, format)?;
        }
        crate::app::BoardAction::Find { project, name } => {
            let boards = client.find_boards(&project, &name).await?;
            render_collection(&boards, format, full);
        }
        crate::app::BoardAction::Create { project, name } => {
            let params = CreateBoard {
                project_id: project.clone(),
                name,
                board_type: "kanban".to_string(),
                position: 65536.0,
            };
            let board = client.create_board(&project, params).await?;
            render_item(&board, format, full);
        }
        crate::app::BoardAction::Update { id, name } => {
            if name.is_none() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            let board = client.update_board(&id, UpdateBoard { name }).await?;
            render_item(&board, format, full);
        }
        crate::app::BoardAction::Delete { id } => {
            if !yes && !confirm_delete("board", &id) {
                render_message("Aborted.", format);
                return Ok(());
            }
            client.delete_board(&id).await?;
            render_message("Board deleted.", format);
        }
    }
    Ok(())
}
