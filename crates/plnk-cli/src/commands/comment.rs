use plnk_core::api::CommentApi;
use plnk_core::error::PlankaError;
use plnk_core::models::{CreateComment, UpdateComment};

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::input::resolve_text;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

pub async fn execute(
    client: &(impl CommentApi + NameSource),
    action: crate::app::CommentAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::CommentAction::List { card } => {
            let comments = client.list_comments(&card).await?;
            render_collection_named(client, &comments, format, full, names).await?;
        }
        crate::app::CommentAction::Create { card, text } => {
            let resolved = resolve_text(&text)?;
            let params = CreateComment { text: resolved };
            let comment = client.create_comment(&card, params).await?;
            render_item_named(client, &comment, format, full, names).await?;
        }
        crate::app::CommentAction::Update { id, text } => {
            let resolved = resolve_text(&text)?;
            let params = UpdateComment { text: resolved };
            let comment = client.update_comment(&id, params).await?;
            render_item_named(client, &comment, format, full, names).await?;
        }
        crate::app::CommentAction::Delete { id } => {
            if !yes && !confirm_delete("comment", &id) {
                render_message("Aborted.", format)?;
                return Ok(());
            }
            client.delete_comment(&id).await?;
            render_message("Comment deleted.", format)?;
        }
    }
    Ok(())
}
