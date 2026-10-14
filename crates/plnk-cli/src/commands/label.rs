use plnk_core::error::PlankaError;
use plnk_core::models::UpdateLabel;

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

pub async fn execute(
    client: &impl NameSource,
    action: crate::app::LabelAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::LabelAction::List { board } => {
            let board = names.resolve_board(client, &board, None).await?;
            let labels = client.list_labels(&board).await?;
            render_collection_named(client, &labels, format, full, names).await?;
        }
        crate::app::LabelAction::Find { board, name } => {
            let board = names.resolve_board(client, &board, None).await?;
            let labels = client.find_labels(&board, &name).await?;
            render_collection_named(client, &labels, format, full, names).await?;
        }
        crate::app::LabelAction::Create { board, name, color } => {
            let board = names.resolve_board(client, &board, None).await?;
            let label = client.create_label(&board, &name, &color).await?;
            render_item_named(client, &label, format, full, names).await?;
        }
        crate::app::LabelAction::Update { id, name, color } => {
            if name.is_none() && color.is_none() {
                return Err(PlankaError::InvalidOptionValue {
                    field: "--name / --color".to_string(),
                    message: "At least one field must be provided for update".to_string(),
                });
            }
            let params = UpdateLabel { name, color };
            let label = client.update_label(&id, params).await?;
            render_item_named(client, &label, format, full, names).await?;
        }
        crate::app::LabelAction::Delete { id } => {
            if !yes && !confirm_delete("label", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_label(&id).await?;
            render_message("Label deleted.", format)?;
        }
    }
    Ok(())
}
