use plnk_core::api::TaskApi;
use plnk_core::error::PlankaError;
use plnk_core::models::UpdateTask;

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

pub async fn execute(
    client: &(impl TaskApi + NameSource),
    action: crate::app::TaskAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::TaskAction::List { card } => {
            let tasks = client.list_tasks(&card).await?;
            render_collection_named(client, &tasks, format, full, names).await?;
        }
        crate::app::TaskAction::Create { card, title } => {
            let task = client.create_task(&card, &title).await?;
            render_item_named(client, &task, format, full, names).await?;
        }
        crate::app::TaskAction::Update { id, title } => {
            if title.is_none() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--title".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            let params = UpdateTask {
                name: title,
                is_completed: None,
            };
            let task = client.update_task(&id, params).await?;
            render_item_named(client, &task, format, full, names).await?;
        }
        crate::app::TaskAction::Complete { id } => {
            let task = client.complete_task(&id).await?;
            render_item_named(client, &task, format, full, names).await?;
        }
        crate::app::TaskAction::Reopen { id } => {
            let task = client.reopen_task(&id).await?;
            render_item_named(client, &task, format, full, names).await?;
        }
        crate::app::TaskAction::Delete { id } => {
            if !yes && !confirm_delete("task", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_task(&id).await?;
            render_message("Task deleted.", format)?;
        }
    }
    Ok(())
}
