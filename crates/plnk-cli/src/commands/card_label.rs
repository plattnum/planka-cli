use plnk_core::api::CardLabelApi;
use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

pub async fn execute(
    client: &(impl CardLabelApi + NameSource),
    action: crate::app::CardLabelAction,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    match action {
        crate::app::CardLabelAction::List { card } => {
            let labels = client.list_card_labels(&card).await?;
            render_collection_named(client, &labels, format, full, names).await?;
        }
        crate::app::CardLabelAction::Add { card, label } => {
            let label = names.resolve_label(client, &label, None).await?;
            let card_label = client.add_card_label(&card, &label).await?;
            render_item_named(client, &card_label, format, full, names).await?;
        }
        crate::app::CardLabelAction::Remove { card, label } => {
            let label = names.resolve_label(client, &label, None).await?;
            client.remove_card_label(&card, &label).await?;
            render_message("Card label removed.", format)?;
        }
    }
    Ok(())
}
