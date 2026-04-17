use plnk_core::api::CardLabelApi;
use plnk_core::error::PlankaError;

use crate::app::OutputFormat;
use crate::output::{render_collection, render_item, render_message};

pub async fn execute(
    client: &impl CardLabelApi,
    action: crate::app::CardLabelAction,
    format: OutputFormat,
    full: bool,
) -> Result<(), PlankaError> {
    match action {
        crate::app::CardLabelAction::List { card } => {
            let labels = client.list_card_labels(&card).await?;
            render_collection(&labels, format, full)?;
        }
        crate::app::CardLabelAction::Add { card, label } => {
            let card_label = client.add_card_label(&card, &label).await?;
            render_item(&card_label, format, full)?;
        }
        crate::app::CardLabelAction::Remove { card, label } => {
            client.remove_card_label(&card, &label).await?;
            render_message("Card label removed.", format)?;
        }
    }
    Ok(())
}
