use plnk_core::error::PlankaError;
use plnk_core::models::{CreateProject, UpdateProject};

use crate::app::OutputFormat;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message, render_snapshot};

pub async fn execute(
    client: &impl NameSource,
    action: crate::app::ProjectAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::ProjectAction::List => {
            let projects = client.list_projects().await?;
            render_collection_named(client, &projects, format, full, names).await?;
        }
        crate::app::ProjectAction::Get { id } => {
            let project = client.get_project(&id).await?;
            render_item_named(client, &project, format, full, names).await?;
        }
        crate::app::ProjectAction::Snapshot { id } => {
            let snapshot = client.get_project_snapshot(&id).await?;
            render_snapshot(&snapshot, format)?;
        }
        crate::app::ProjectAction::Find { name } => {
            let projects = client.find_projects(&name).await?;
            render_collection_named(client, &projects, format, full, names).await?;
        }
        crate::app::ProjectAction::Create { name } => {
            let project = client
                .create_project(CreateProject {
                    name,
                    project_type: "private".to_string(),
                })
                .await?;
            render_item_named(client, &project, format, full, names).await?;
        }
        crate::app::ProjectAction::Update { id, name } => {
            if name.is_none() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            let project = client.update_project(&id, UpdateProject { name }).await?;
            render_item_named(client, &project, format, full, names).await?;
        }
        crate::app::ProjectAction::Delete { id } => {
            if !yes && !confirm_delete("project", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_project(&id).await?;
            render_message("Project deleted.", format)?;
        }
    }
    Ok(())
}

/// Prompt user to confirm a destructive operation.
pub fn confirm_delete(resource_type: &str, id: &str) -> bool {
    eprint!("Delete {resource_type} {id}? [y/N] ");
    let mut buf = String::new();
    if std::io::stdin().read_line(&mut buf).is_err() {
        return false;
    }
    matches!(buf.trim(), "y" | "Y" | "yes" | "Yes")
}
