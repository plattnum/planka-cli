use plnk_core::api::CustomFieldApi;
use plnk_core::error::PlankaError;
use plnk_core::models::UpdateCustomField;

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::output::{render_collection, render_item, render_message};

/// Resolve the `--group` / `--base-group` pair into a group ID and a flag
/// saying which kind it is.
///
/// The kind cannot be inferred from the ID: base groups and ordinary groups are
/// separate wire resources reached by different routes, and a base group's
/// fields are readable only through the projects list.
fn scope(group: Option<String>, base_group: Option<String>) -> Result<(String, bool), PlankaError> {
    match (group, base_group) {
        (Some(id), None) => Ok((id, false)),
        (None, Some(id)) => Ok((id, true)),
        _ => Err(PlankaError::MissingRequiredOption {
            field: "--group or --base-group".to_string(),
        }),
    }
}

pub async fn execute(
    client: &impl CustomFieldApi,
    action: crate::app::FieldAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    use crate::app::FieldAction as Action;

    match action {
        Action::List { group, base_group } => {
            let (id, base) = scope(group, base_group)?;
            let fields = client.list_fields(&id, base).await?;
            render_collection(&fields, format, full)?;
        }
        Action::Find {
            group,
            base_group,
            name,
        } => {
            let (id, base) = scope(group, base_group)?;
            let fields = client.find_fields(&id, base, &name).await?;
            render_collection(&fields, format, full)?;
        }
        Action::Create {
            group,
            base_group,
            name,
            show_on_front,
        } => {
            let (id, base) = scope(group, base_group)?;
            let field = client.create_field(&id, base, &name, show_on_front).await?;
            render_item(&field, format, full)?;
        }
        Action::Update {
            id,
            name,
            show_on_front,
        } => {
            if name.is_none() && show_on_front.is_none() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name / --show-on-front".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            let params = UpdateCustomField {
                name,
                show_on_front_of_card: show_on_front,
            };
            let field = client.update_field(&id, params).await?;
            render_item(&field, format, full)?;
        }
        Action::Delete { id } => {
            if !yes && !confirm_delete("field", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_field(&id).await?;
            render_message("Field deleted.", format)?;
        }
    }
    Ok(())
}
