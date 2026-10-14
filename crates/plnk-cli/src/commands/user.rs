use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named};

pub async fn execute(
    client: &impl NameSource,
    action: crate::app::UserAction,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::UserAction::List => {
            let users = client.list_users().await?;
            render_collection_named(client, &users, format, full, names).await?;
        }
        crate::app::UserAction::Get { id } => {
            let user = client.get_user(&id).await?;
            render_item_named(client, &user, format, full, names).await?;
        }
    }
    Ok(())
}
