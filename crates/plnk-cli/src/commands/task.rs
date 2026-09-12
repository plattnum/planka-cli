use plnk_core::api::{TaskApi, TaskListApi, match_by_name};
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateTaskList, TaskList, UpdateTask};

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::output::{render_collection, render_item, render_message};

fn listed(candidates: &[TaskList]) -> String {
    candidates
        .iter()
        .map(|tl| format!("{} ({})", tl.name, tl.id))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Resolve `--task-list` to one of the card's task lists. An argument that is
/// already one of the card's task list IDs skips name matching entirely.
async fn resolve_task_list(
    client: &impl TaskListApi,
    card_id: &str,
    query: &str,
) -> Result<TaskList, PlankaError> {
    let lists = client.list_task_lists(card_id).await?;

    if let Some(found) = lists.iter().find(|tl| tl.id == query) {
        return Ok(found.clone());
    }

    let matched = match_by_name(&lists, query);
    match matched.len() {
        0 => Err(PlankaError::NotFoundMessage {
            message: format!(
                "No task list matching '{query}' on this card. \
                 Use 'plnk task-list list --card {card_id}' to inspect the card's \
                 task lists or pass a task list ID."
            ),
        }),
        1 => Ok(matched[0].clone()),
        _ => {
            let candidates: Vec<TaskList> = matched.into_iter().cloned().collect();
            Err(PlankaError::InvalidOptionValue {
                field: "--task-list".to_string(),
                message: format!(
                    "'{query}' matched multiple task lists. Pass an ID. Matches: {}",
                    listed(&candidates)
                ),
            })
        }
    }
}

/// Decide which task list a new task goes into.
///
/// With `--task-list` the answer is whatever it resolves to. Without it:
/// no lists → create a default "Tasks" list; one list → use it; more than
/// one → refuse, because guessing would silently misplace the task.
async fn select_task_list(
    client: &impl TaskListApi,
    card_id: &str,
    query: Option<&str>,
) -> Result<String, PlankaError> {
    if let Some(query) = query {
        return Ok(resolve_task_list(client, card_id, query).await?.id);
    }

    let lists = client.list_task_lists(card_id).await?;
    match lists.as_slice() {
        [] => {
            let created = client
                .create_task_list(
                    card_id,
                    CreateTaskList {
                        name: "Tasks".to_string(),
                        position: 65536.0,
                        show_on_front_of_card: None,
                        hide_completed_tasks: None,
                    },
                )
                .await?;
            Ok(created.id)
        }
        [only] => Ok(only.id.clone()),
        many => Err(PlankaError::InvalidOptionValue {
            field: "--task-list".to_string(),
            message: format!(
                "Card has {} task lists; pass --task-list to choose one. Candidates: {}",
                many.len(),
                listed(many)
            ),
        }),
    }
}

pub async fn execute(
    client: &(impl TaskApi + TaskListApi),
    action: crate::app::TaskAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::TaskAction::List { card, task_list } => {
            let tasks = match task_list {
                Some(query) => {
                    let tl = resolve_task_list(client, &card, &query).await?;
                    client.list_tasks_in_list(&tl.id).await?
                }
                None => client.list_tasks(&card).await?,
            };
            render_collection(&tasks, format, full)?;
        }
        crate::app::TaskAction::Create {
            card,
            title,
            task_list,
        } => {
            let task_list_id = select_task_list(client, &card, task_list.as_deref()).await?;
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
