use plnk_core::api::CardApi;
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateCard, FindScope, MoveCard, UpdateCard};

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::input::resolve_text;
use crate::output::{render_collection, render_item, render_message};

/// Parse position flag: "top", "bottom", or numeric value.
fn parse_position(pos: &str) -> Result<f64, PlankaError> {
    match pos.to_lowercase().as_str() {
        "top" => Ok(0.0),
        "bottom" => Ok(f64::MAX),
        other => other
            .parse::<f64>()
            .map_err(|_| PlankaError::InvalidOptionValue {
                field: "--position".to_string(),
                message: format!("Must be 'top', 'bottom', or a number, got '{other}'"),
            }),
    }
}

#[allow(clippy::too_many_lines)]
pub async fn execute(
    client: &impl CardApi,
    action: crate::app::CardAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::CardAction::List { list } => {
            let cards = client.list_cards(&list).await?;
            render_collection(&cards, format, full);
        }
        crate::app::CardAction::Get { id } => {
            let card = client.get_card(&id).await?;
            render_item(&card, format, full);
        }
        crate::app::CardAction::Find {
            list,
            board,
            project,
            title,
        } => {
            let scope = if let Some(list_id) = list {
                FindScope::List(list_id)
            } else if let Some(board_id) = board {
                FindScope::Board(board_id)
            } else if let Some(project_id) = project {
                FindScope::Project(project_id)
            } else {
                return Err(PlankaError::MissingRequiredOption {
                    field: "--list, --board, or --project".to_string(),
                });
            };
            let cards = client.find_cards(scope, &title).await?;
            render_collection(&cards, format, full);
        }
        crate::app::CardAction::Create {
            list,
            title,
            description,
            position,
        } => {
            let desc = match description {
                Some(raw) => Some(resolve_text(&raw)?),
                None => None,
            };
            let pos = match position {
                Some(p) => parse_position(&p)?,
                None => 65536.0,
            };
            let params = CreateCard {
                list_id: list.clone(),
                name: title,
                description: desc,
                card_type: "project".to_string(),
                position: pos,
            };
            let card = client.create_card(&list, params).await?;
            render_item(&card, format, full);
        }
        crate::app::CardAction::Update {
            id,
            title,
            description,
        } => {
            if title.is_none() && description.is_none() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--title / --description".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            let desc = match description {
                Some(raw) => Some(resolve_text(&raw)?),
                None => None,
            };
            let params = UpdateCard {
                name: title,
                description: desc,
                due_date: None,
                is_closed: None,
            };
            let card = client.update_card(&id, params).await?;
            render_item(&card, format, full);
        }
        crate::app::CardAction::Move {
            id,
            to_list,
            position,
        } => {
            let pos = match position {
                Some(p) => parse_position(&p)?,
                None => 65536.0,
            };
            let params = MoveCard {
                list_id: to_list,
                position: pos,
            };
            let card = client.move_card(&id, params).await?;
            render_item(&card, format, full);
        }
        crate::app::CardAction::Archive { id } => {
            let card = client.archive_card(&id).await?;
            render_item(&card, format, full);
        }
        crate::app::CardAction::Unarchive { id } => {
            let card = client.unarchive_card(&id).await?;
            render_item(&card, format, full);
        }
        crate::app::CardAction::Delete { id } => {
            if !yes && !confirm_delete("card", &id) {
                render_message("Aborted.", format);
                return Ok(());
            }
            client.delete_card(&id).await?;
            render_message("Card deleted.", format);
        }
    }
    Ok(())
}
