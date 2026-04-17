use plnk_core::api::LabelApi;
use plnk_core::error::PlankaError;
use plnk_core::models::UpdateLabel;

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::output::{render_collection, render_item, render_message};

pub async fn execute(
    client: &impl LabelApi,
    action: crate::app::LabelAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::LabelAction::List { board } => {
            let labels = client.list_labels(&board).await?;
            render_collection(&labels, format, full)?;
        }
        crate::app::LabelAction::Find { board, name } => {
            let labels = client.find_labels(&board, &name).await?;
            render_collection(&labels, format, full)?;
        }
        crate::app::LabelAction::Create { board, name, color } => {
            let label = client.create_label(&board, &name, &color).await?;
            render_item(&label, format, full)?;
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
            render_item(&label, format, full)?;
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
