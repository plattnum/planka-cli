use plnk_core::api::AssigneeApi;
use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::output::{render_collection, render_item, render_message};

pub async fn execute(
    client: &impl AssigneeApi,
    action: crate::app::CardAssigneeAction,
    format: OutputFormat,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::CardAssigneeAction::List { card } => {
            let assignees = client.list_assignees(&card).await?;
            render_collection(&assignees, format, full);
        }
        crate::app::CardAssigneeAction::Add { card, user } => {
            let assignee = client.add_assignee(&card, &user).await?;
            render_item(&assignee, format, full);
        }
        crate::app::CardAssigneeAction::Remove { card, user } => {
            client.remove_assignee(&card, &user).await?;
            render_message("Assignee removed.", format);
        }
    }
    Ok(())
}
