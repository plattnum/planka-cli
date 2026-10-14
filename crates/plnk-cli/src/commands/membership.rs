use plnk_core::api::MembershipApi;
use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

pub async fn execute(
    client: &(impl MembershipApi + NameSource),
    action: crate::app::MembershipAction,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::MembershipAction::List { project, board } => {
            let project = names.resolve_optional_project(client, project).await?;
            let board = names
                .resolve_optional_board(client, board, project.as_deref())
                .await?;
            if let Some(board_id) = board {
                let members = client.list_board_members(&board_id).await?;
                render_collection_named(client, &members, format, full, names).await?;
            } else if let Some(project_id) = project {
                let managers = client.list_project_managers(&project_id).await?;
                render_collection_named(client, &managers, format, full, names).await?;
            } else {
                return Err(PlankaError::MissingRequiredOption {
                    field: "--project or --board".to_string(),
                });
            }
        }
        crate::app::MembershipAction::Add {
            project,
            board,
            user,
            role,
        } => {
            let project = names.resolve_optional_project(client, project).await?;
            let board = names
                .resolve_optional_board(client, board, project.as_deref())
                .await?;
            let user = names.resolve_user(client, &user).await?;
            if let Some(board_id) = board {
                let member = client
                    .add_board_member(&board_id, &user, role.as_deref())
                    .await?;
                render_item_named(client, &member, format, full, names).await?;
            } else if let Some(project_id) = project {
                let manager = client.add_project_manager(&project_id, &user).await?;
                render_item_named(client, &manager, format, full, names).await?;
            } else {
                return Err(PlankaError::MissingRequiredOption {
                    field: "--project or --board".to_string(),
                });
            }
        }
        crate::app::MembershipAction::Remove {
            project,
            board,
            user,
        } => {
            let project = names.resolve_optional_project(client, project).await?;
            let board = names
                .resolve_optional_board(client, board, project.as_deref())
                .await?;
            let user = names.resolve_user(client, &user).await?;
            if let Some(board_id) = board {
                // Find the membership ID by listing and matching user
                let members = client.list_board_members(&board_id).await?;
                let member = members.iter().find(|m| m.user_id == user);
                if let Some(m) = member {
                    client.remove_board_member(&m.id).await?;
                    render_message("Board member removed.", format)?;
                } else {
                    return Err(PlankaError::NotFound {
                        resource_type: "board membership".to_string(),
                        id: user,
                    });
                }
            } else if let Some(project_id) = project {
                // Find the project manager by listing and matching user
                let managers = client.list_project_managers(&project_id).await?;
                let manager = managers.iter().find(|m| m.user_id == user);
                if let Some(m) = manager {
                    client.remove_project_manager(&m.id).await?;
                    render_message("Project manager removed.", format)?;
                } else {
                    return Err(PlankaError::NotFound {
                        resource_type: "project manager".to_string(),
                        id: user,
                    });
                }
            } else {
                return Err(PlankaError::MissingRequiredOption {
                    field: "--project or --board".to_string(),
                });
            }
        }
    }
    Ok(())
}
