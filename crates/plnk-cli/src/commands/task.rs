use plnk_core::api::{TaskApi, TaskListApi};
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateTaskList, UpdateTask};

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::output::{render_collection, render_item, render_message};

pub async fn execute(
    client: &(impl TaskApi + TaskListApi),
    action: crate::app::TaskAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::TaskAction::List { card } => {
            let tasks = client.list_tasks(&card).await?;
            render_collection(&tasks, format, full)?;
        }
        crate::app::TaskAction::Create { card, title } => {
            // Tasks live inside task lists. Use the card's first list, or
            // create a default one when the card has none.
            let lists = client.list_task_lists(&card).await?;
            let task_list_id = match lists.first() {
                Some(tl) => tl.id.clone(),
                None => {
                    client
                        .create_task_list(
                            &card,
                            CreateTaskList {
                                name: "Tasks".to_string(),
                                position: 65536.0,
                                show_on_front_of_card: None,
                                hide_completed_tasks: None,
                            },
                        )
                        .await?
                        .id
                }
            };
            let task = client.create_task(&task_list_id, &title).await?;
            render_item(&task, format, full)?;
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
            render_item(&task, format, full)?;
        }
        crate::app::TaskAction::Complete { id } => {
            let task = client.complete_task(&id).await?;
            render_item(&task, format, full)?;
        }
        crate::app::TaskAction::Reopen { id } => {
            let task = client.reopen_task(&id).await?;
            render_item(&task, format, full)?;
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
