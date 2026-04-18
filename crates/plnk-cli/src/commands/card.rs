use plnk_core::api::{CardApi, LabelApi, ListApi, match_by_name};
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateCard, FindScope, Label, MoveCard, UpdateCard};

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::input::resolve_text;
use crate::output::{render_collection, render_item, render_message, render_snapshot};

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

fn resolve_label_id(labels: &[Label], query: &str) -> Result<String, PlankaError> {
    if labels.iter().any(|label| label.id == query) {
        return Ok(query.to_string());
    }

    let matched = match_by_name(labels, query);
    match matched.len() {
        0 => Err(PlankaError::NotFoundMessage {
            message: format!(
                "No label matching '{query}' was found on this board. \
                 Use 'plnk label list --board <boardId>' to inspect available labels \
                 or pass a label ID."
            ),
        }),
        1 => Ok(matched[0].id.clone()),
        _ => {
            let candidates = matched
                .iter()
                .map(|label| {
                    let name = label.name.as_deref().unwrap_or("<unnamed>");
                    format!("{name} ({})", label.id)
                })
                .collect::<Vec<_>>()
                .join(", ");
            Err(PlankaError::InvalidOptionValue {
                field: "--label".to_string(),
                message: format!(
                    "Label '{query}' matched multiple labels on this board. \
                     Be more specific or use a label ID. Matches: {candidates}"
                ),
            })
        }
    }
}

async fn resolve_label_scope(
    client: &(impl LabelApi + ListApi),
    list: Option<&str>,
    board: Option<&str>,
    labels: &[String],
) -> Result<Option<(String, Option<String>, Vec<String>)>, PlankaError> {
    if labels.is_empty() {
        return Ok(None);
    }

    let (board_id, list_id) = if let Some(list_id) = list {
        let list_item = client.get_list(list_id).await?;
        (list_item.board_id, Some(list_id.to_string()))
    } else if let Some(board_id) = board {
        (board_id.to_string(), None)
    } else {
        return Err(PlankaError::InvalidOptionValue {
            field: "--label".to_string(),
            message: "--label is only supported with --list or --board scopes because labels are board-scoped".to_string(),
        });
    };

    let board_labels = client.list_labels(&board_id).await?;
    let resolved = labels
        .iter()
        .map(|query| resolve_label_id(&board_labels, query))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some((board_id, list_id, resolved)))
}

#[allow(clippy::too_many_lines)]
pub async fn execute(
    client: &(impl CardApi + LabelApi + ListApi),
    action: crate::app::CardAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::CardAction::List { list, board, label } => {
            let cards = if let Some((board_id, list_id, label_ids)) =
                resolve_label_scope(client, list.as_deref(), board.as_deref(), &label).await?
            {
                client
                    .list_cards_in_board(&board_id, list_id.as_deref(), &label_ids)
                    .await?
            } else if let Some(list_id) = list {
                client.list_cards(&list_id).await?
            } else if let Some(board_id) = board {
                client.list_cards_in_board(&board_id, None, &[]).await?
            } else {
                return Err(PlankaError::MissingRequiredOption {
                    field: "--list or --board".to_string(),
                });
            };
            render_collection(&cards, format, full)?;
        }
        crate::app::CardAction::Get { id } => {
            let card = client.get_card(&id).await?;
            render_item(&card, format, full)?;
        }
        crate::app::CardAction::Snapshot { id } => {
            let snapshot = client.get_card_snapshot(&id).await?;
            render_snapshot(&snapshot, format)?;
        }
        crate::app::CardAction::Find {
            list,
            board,
            project,
            title,
            label,
        } => {
            if title.is_none() && label.is_empty() {
                return Err(PlankaError::MissingRequiredOption {
                    field: "--title or --label".to_string(),
                });
            }

            let cards = if let Some((board_id, list_id, label_ids)) =
                resolve_label_scope(client, list.as_deref(), board.as_deref(), &label).await?
            {
                let scoped_cards = client
                    .list_cards_in_board(&board_id, list_id.as_deref(), &label_ids)
                    .await?;
                if let Some(title) = title.as_deref() {
                    match_by_name(&scoped_cards, title)
                        .into_iter()
                        .cloned()
                        .collect()
                } else {
                    scoped_cards
                }
            } else {
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
                client
                    .find_cards(scope, title.as_deref().unwrap_or(""))
                    .await?
            };
            render_collection(&cards, format, full)?;
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
            render_item(&card, format, full)?;
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
            render_item(&card, format, full)?;
        }
        crate::app::CardAction::Move {
            id,
            to_list,
            to_board,
            position,
        } => {
            let pos = match position {
                Some(p) => parse_position(&p)?,
                None => 65536.0,
            };
            let params = MoveCard {
                board_id: to_board,
                list_id: to_list,
                position: pos,
            };
            let card = client.move_card(&id, params).await?;
            render_item(&card, format, full)?;
        }
        crate::app::CardAction::Archive { id } => {
            let card = client.archive_card(&id).await?;
            render_item(&card, format, full)?;
        }
        crate::app::CardAction::Unarchive { id } => {
            let card = client.unarchive_card(&id).await?;
            render_item(&card, format, full)?;
        }
        crate::app::CardAction::Delete { id } => {
            if !yes && !confirm_delete("card", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_card(&id).await?;
            render_message("Card deleted.", format)?;
        }
        // Label and Assignee subcommands are dispatched in main.rs
        crate::app::CardAction::Label(_) | crate::app::CardAction::Assignee(_) => {
            unreachable!("card label/assignee dispatched in main.rs")
        }
    }
    Ok(())
}
