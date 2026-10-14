use plnk_core::error::PlankaError;
use plnk_core::models::{CreateList, UpdateList};

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

pub async fn execute(
    client: &impl NameSource,
    action: crate::app::ListAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::ListAction::List { board } => {
            let board = names.resolve_board(client, &board, None).await?;
            let lists = client.list_lists(&board).await?;
            render_collection_named(client, &lists, format, full, names).await?;
        }
        crate::app::ListAction::Get { id } => {
            let list = client.get_list(&id).await?;
            render_item_named(client, &list, format, full, names).await?;
        }
        crate::app::ListAction::Find { board, name } => {
            let board = names.resolve_board(client, &board, None).await?;
            let lists = client.find_lists(&board, &name).await?;
            render_collection_named(client, &lists, format, full, names).await?;
        }
        crate::app::ListAction::Create { board, name } => {
            let board = names.resolve_board(client, &board, None).await?;
            let params = CreateList {
                board_id: board.clone(),
                name,
                list_type: "active".to_string(),
                position: 65536.0,
            };
            let list = client.create_list(&board, params).await?;
            render_item_named(client, &list, format, full, names).await?;
        }
        crate::app::ListAction::Update { id, name, position } => {
            if name.is_none() && position.is_none() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name / --position".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            let list = client
                .update_list(&id, UpdateList { name, position })
                .await?;
            render_item_named(client, &list, format, full, names).await?;
        }
        crate::app::ListAction::Move { id, to_position } => {
            let list = client
                .update_list(
                    &id,
                    UpdateList {
                        name: None,
                        position: Some(to_position),
                    },
                )
                .await?;
            render_item_named(client, &list, format, full, names).await?;
        }
        crate::app::ListAction::Delete { id } => {
            if !yes && !confirm_delete("list", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_list(&id).await?;
            render_message("List deleted.", format)?;
        }
    }
    Ok(())
}
