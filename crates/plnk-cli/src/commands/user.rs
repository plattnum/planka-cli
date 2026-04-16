use plnk_core::api::UserApi;
use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::output::{render_collection, render_item};

pub async fn execute(
    client: &impl UserApi,
    action: crate::app::UserAction,
    format: OutputFormat,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::UserAction::List => {
            let users = client.list_users().await?;
            render_collection(&users, format, full);
        }
        crate::app::UserAction::Get { id } => {
            let user = client.get_user(&id).await?;
            render_item(&user, format, full);
        }
    }
    Ok(())
}
