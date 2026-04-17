use std::path::PathBuf;

use plnk_core::api::AttachmentApi;
use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::output::{render_collection, render_item, render_message};

pub async fn execute(
    client: &impl AttachmentApi,
    action: crate::app::AttachmentAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::AttachmentAction::List { card } => {
            let attachments = client.list_attachments(&card).await?;
            render_collection(&attachments, format, full)?;
        }
        crate::app::AttachmentAction::Upload { card, file } => {
            let path = PathBuf::from(&file);
            if !path.exists() {
                return Err(PlankaError::FileReadError {
                    path: file,
                    source: std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"),
                });
            }
            let attachment = client.upload_attachment(&card, &path).await?;
            render_item(&attachment, format, full)?;
        }
        crate::app::AttachmentAction::Download { id, card, out } => {
            let out_path = out.as_deref().map(std::path::Path::new);
            let saved_to = client.download_attachment(&card, &id, out_path).await?;
            render_message(&format!("Downloaded to {}", saved_to.display()), format)?;
        }
        crate::app::AttachmentAction::Delete { id } => {
            if !yes && !confirm_delete("attachment", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_attachment(&id).await?;
            render_message("Attachment deleted.", format)?;
        }
    }
    Ok(())
}
