use plnk_core::error::PlankaError;
use plnk_core::models::UpdateCustomField;

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

/// Resolve the `--group` / `--base-group` pair into a group ID and a flag
/// saying which kind it is.
///
/// The kind cannot be inferred from the ID: base groups and ordinary groups are
/// separate wire resources reached by different routes, and a base group's
/// fields are readable only through the projects list.
async fn scope<C>(
    client: &C,
    names: &mut NameIndex,
    group: Option<String>,
    base_group: Option<String>,
) -> Result<(String, bool), PlankaError>
where
    C: NameSource + ?Sized,
{
    match (group, base_group) {
        (Some(group), None) => Ok((names.resolve_group(client, &group).await?, false)),
        (None, Some(base_group)) => {
            Ok((names.resolve_base_group(client, &base_group).await?, true))
        }
        _ => Err(PlankaError::MissingRequiredOption {
            field: "--group or --base-group".to_string(),
        }),
    }
}

pub async fn execute(
    client: &impl NameSource,
    action: crate::app::FieldAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    use crate::app::FieldAction as Action;

    match action {
        Action::List { group, base_group } => {
            let (id, base) = scope(client, names, group, base_group).await?;
            let fields = client.list_fields(&id, base).await?;
            render_collection_named(client, &fields, format, full, names).await?;
        }
        Action::Find {
            group,
            base_group,
            name,
        } => {
            let (id, base) = scope(client, names, group, base_group).await?;
            let fields = client.find_fields(&id, base, &name).await?;
            render_collection_named(client, &fields, format, full, names).await?;
        }
        Action::Create {
            group,
            base_group,
            name,
            show_on_front,
        } => {
            let (id, base) = scope(client, names, group, base_group).await?;
            let field = client.create_field(&id, base, &name, show_on_front).await?;
            render_item_named(client, &field, format, full, names).await?;
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
            render_item_named(client, &field, format, full, names).await?;
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
