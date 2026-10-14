use plnk_core::api::AssigneeApi;
use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

pub async fn execute(
    client: &(impl AssigneeApi + NameSource),
    action: crate::app::CardAssigneeAction,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::CardAssigneeAction::List { card } => {
            let assignees = client.list_assignees(&card).await?;
            render_collection_named(client, &assignees, format, full, names).await?;
        }
        crate::app::CardAssigneeAction::Add { card, user } => {
            let user = names.resolve_user(client, &user).await?;
            let assignee = client.add_assignee(&card, &user).await?;
            render_item_named(client, &assignee, format, full, names).await?;
        }
        crate::app::CardAssigneeAction::Remove { card, user } => {
            let user = names.resolve_user(client, &user).await?;
            client.remove_assignee(&card, &user).await?;
            render_message("Assignee removed.", format)?;
        }
    }
    Ok(())
}
