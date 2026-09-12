use plnk_core::api::TaskListApi;
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateTaskList, UpdateTaskList};

use crate::app::OutputFormat;
use crate::commands::card::parse_position;
use crate::commands::project::confirm_delete;
use crate::output::{render_collection, render_item, render_message};

pub async fn execute(
    client: &impl TaskListApi,
    action: crate::app::TaskListAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::TaskListAction::List { card } => {
            let lists = client.list_task_lists(&card).await?;
            render_collection(&lists, format, full)?;
        }
        crate::app::TaskListAction::Find { card, name } => {
            let lists = client.find_task_lists(&card, &name).await?;
            render_collection(&lists, format, full)?;
        }
        crate::app::TaskListAction::Get { id } => {
            let list = client.get_task_list(&id).await?;
            render_item(&list, format, full)?;
        }
        crate::app::TaskListAction::Create {
            card,
            name,
            position,
            show_on_front,
            hide_completed,
        } => {
            if name.trim().is_empty() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name".to_string(),
                    message: "Task list name must not be empty".to_string(),
                });
            }
            let position = match position {
                Some(p) => parse_position(&p)?,
                None => 65536.0,
            };
            let params = CreateTaskList {
                name,
                position,
                show_on_front_of_card: show_on_front,
                hide_completed_tasks: hide_completed,
            };
            let list = client.create_task_list(&card, params).await?;
            render_item(&list, format, full)?;
        }
        crate::app::TaskListAction::Update {
            id,
            name,
            show_on_front,
            hide_completed,
        } => {
            if name.is_none() && show_on_front.is_none() && hide_completed.is_none() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name / --show-on-front / --hide-completed".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            if name.as_deref().is_some_and(|n| n.trim().is_empty()) {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name".to_string(),
                    message: "Task list name must not be empty".to_string(),
                });
            }
            let params = UpdateTaskList {
                name,
                show_on_front_of_card: show_on_front,
                hide_completed_tasks: hide_completed,
                ..UpdateTaskList::default()
            };
            let list = client.update_task_list(&id, params).await?;
            render_item(&list, format, full)?;
        }
        crate::app::TaskListAction::Move { id, to_position } => {
            let params = UpdateTaskList {
                position: Some(parse_position(&to_position)?),
                ..UpdateTaskList::default()
            };
            let list = client.update_task_list(&id, params).await?;
            render_item(&list, format, full)?;
        }
        crate::app::TaskListAction::Delete { id } => {
            // Deleting a task list deletes every task in it, so the prompt
            // names the consequence rather than just the resource.
            if !yes && !confirm_delete("task list (and all of its tasks)", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_task_list(&id).await?;
            render_message("Task list deleted.", format)?;
        }
    }
    Ok(())
}
