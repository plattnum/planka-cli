use plnk_core::api::ListApi;
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateList, UpdateList};

use crate::app::OutputFormat;
use crate::commands::card::parse_position;
use crate::commands::project::confirm_delete;
use crate::output::{render_collection, render_item, render_message};

pub async fn execute(
    client: &impl ListApi,
    action: crate::app::ListAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::ListAction::List { board } => {
            let lists = client.list_lists(&board).await?;
            render_collection(&lists, format, full)?;
        }
        crate::app::ListAction::Get { id } => {
            let list = client.get_list(&id).await?;
            render_item(&list, format, full)?;
        }
        crate::app::ListAction::Find { board, name } => {
            let lists = client.find_lists(&board, &name).await?;
            render_collection(&lists, format, full)?;
        }
        crate::app::ListAction::Create {
            board,
            name,
            position,
            list_type,
            color,
        } => {
            let params = CreateList {
                board_id: board.clone(),
                name,
                list_type: list_type.unwrap_or_else(|| "active".to_string()),
                position: position
                    .as_deref()
                    .map(parse_position)
                    .transpose()?
                    .unwrap_or(65536.0),
            };
            let mut list = client.create_list(&board, params).await?;
            // The create endpoint drops `color`, so set it with a second request.
            if let Some(color) = color {
                let update = UpdateList {
                    color: Some(Some(color)),
                    ..UpdateList::default()
                };
                list = client
                    .update_list(&list.id, update)
                    .await
                    .map_err(|error| color_after_create_failed(&list.id, &error))?;
            }
            render_item(&list, format, full)?;
        }
        crate::app::ListAction::Update {
            id,
            name,
            position,
            list_type,
            color,
            clear_color,
        } => {
            if name.is_none()
                && position.is_none()
                && list_type.is_none()
                && color.is_none()
                && !clear_color
            {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name / --position / --type / --color / --clear-color".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            let color = if clear_color {
                Some(None)
            } else {
                color.map(Some)
            };
            let params = UpdateList {
                name,
                position,
                list_type,
                color,
            };
            let list = client.update_list(&id, params).await?;
            render_item(&list, format, full)?;
        }
        crate::app::ListAction::Move { id, to_position } => {
            let list = client
                .update_list(
                    &id,
                    UpdateList {
                        position: Some(to_position),
                        ..UpdateList::default()
                    },
                )
                .await?;
            render_item(&list, format, full)?;
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

/// Report a failed color update after the list was already created.
///
/// The list exists at this point, so the message names its ID; the exit code
/// stays that of the underlying failure.
fn color_after_create_failed(list_id: &str, error: &PlankaError) -> PlankaError {
    let message = format!("list {list_id} was created but setting its color failed: {error}");
    match error.exit_code() {
        2 => PlankaError::InvalidOptionValue {
            field: "--color".to_string(),
            message,
        },
        3 => PlankaError::AuthenticationFailed { message },
        4 => PlankaError::NotFoundMessage { message },
        _ => PlankaError::ApiError {
            status: match error {
                PlankaError::ApiError { status, .. } => *status,
                _ => 0,
            },
            message,
        },
    }
}
