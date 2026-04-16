use plnk_core::api::MembershipApi;
use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::output::{render_collection, render_item, render_message};

pub async fn execute(
    client: &impl MembershipApi,
    action: crate::app::MembershipAction,
    format: OutputFormat,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::MembershipAction::List { project, board } => {
            if let Some(board_id) = board {
                let members = client.list_board_members(&board_id).await?;
                render_collection(&members, format, full);
            } else if let Some(project_id) = project {
                let managers = client.list_project_managers(&project_id).await?;
                render_collection(&managers, format, full);
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
            if let Some(board_id) = board {
                let member = client
                    .add_board_member(&board_id, &user, role.as_deref())
                    .await?;
                render_item(&member, format, full);
            } else if let Some(project_id) = project {
                let manager = client.add_project_manager(&project_id, &user).await?;
                render_item(&manager, format, full);
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
            if let Some(board_id) = board {
                // Find the membership ID by listing and matching user
                let members = client.list_board_members(&board_id).await?;
                let member = members.iter().find(|m| m.user_id == user);
                if let Some(m) = member {
                    client.remove_board_member(&m.id).await?;
                    render_message("Board member removed.", format);
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
                    render_message("Project manager removed.", format);
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
