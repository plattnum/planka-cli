mod json;
mod markdown;
mod table;

use plnk_core::error::PlankaError;
use plnk_core::models::Tabular;
use serde::Serialize;

use crate::app::OutputFormat;

/// Render a collection of items to stdout.
pub fn render_collection<T: Serialize + Tabular>(
    items: &[T],
    format: OutputFormat,
    full: bool,
) -> Result<(), PlankaError> {
    match format {
        OutputFormat::Json if full => json::print_collection_full(items),
        OutputFormat::Json => json::print_collection_trimmed(items),
        OutputFormat::Table => table::print_collection(items),
        OutputFormat::Markdown => markdown::print_collection(items),
    }
}

/// Render a collection with an explicit JSON `meta` payload.
pub fn render_collection_with_meta<T: Serialize + Tabular, M: Serialize>(
    items: &[T],
    format: OutputFormat,
    full: bool,
    meta: &M,
) -> Result<(), PlankaError> {
    match format {
        OutputFormat::Json if full => json::print_collection_full_with_meta(items, meta),
        OutputFormat::Json => json::print_collection_trimmed_with_meta(items, meta),
        OutputFormat::Table => table::print_collection(items),
        OutputFormat::Markdown => markdown::print_collection(items),
    }
}

/// Render a single item to stdout.
pub fn render_item<T: Serialize + Tabular>(
    item: &T,
    format: OutputFormat,
    full: bool,
) -> Result<(), PlankaError> {
    match format {
        OutputFormat::Json if full => json::print_item_full(item),
        OutputFormat::Json => json::print_item_trimmed(item),
        OutputFormat::Table => table::print_item(item),
        OutputFormat::Markdown => markdown::print_item(item),
    }
}

/// Render an error to stderr (or stdout for JSON format).
pub fn render_error(error: &plnk_core::error::PlankaError, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let envelope = error.to_error_envelope();
            let json = serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| {
                format!(
                    r#"{{"success":false,"error":{{"type":"{}","message":"{}"}}}}"#,
                    error.error_type(),
                    error
                )
            });
            // JSON errors go to stdout for machine parseability
            println!("{json}");
        }
        OutputFormat::Table | OutputFormat::Markdown => {
            eprintln!("Error: {error}");
        }
    }
}

/// Render a simple success message to stdout.
pub fn render_message(message: &str, format: OutputFormat) -> Result<(), PlankaError> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string(&serde_json::json!({
                "success": true,
                "data": { "message": message }
            }))?;
            println!("{json}");
        }
        OutputFormat::Table | OutputFormat::Markdown => {
            println!("{message}");
        }
    }
    Ok(())
}

/// Render a snapshot (the full Planka response verbatim) to stdout.
///
/// Snapshots are heterogeneous nested data (`item` + `included` with many
/// sub-resource types), so only JSON output makes sense. The raw response
/// is placed under the standard `data` envelope unchanged.
pub fn render_snapshot(value: &serde_json::Value, format: OutputFormat) -> Result<(), PlankaError> {
    match format {
        OutputFormat::Json => {
            let envelope = serde_json::json!({
                "success": true,
                "data": value,
            });
            let json = serde_json::to_string_pretty(&envelope)?;
            println!("{json}");
            Ok(())
        }
        OutputFormat::Table | OutputFormat::Markdown => Err(PlankaError::InvalidOptionValue {
            field: "--output".to_string(),
            message: "snapshot only supports --output json (response is nested \
                      heterogeneous data). Use --output json."
                .to_string(),
        }),
    }
}

/// Format a JSON value for display in tables and markdown.
///
/// Nulls become empty strings, booleans become "yes"/"no", numbers and
/// strings render verbatim, arrays/objects fall back to compact JSON.
/// This helper is display-only — it never participates in JSON output.
pub(crate) fn value_to_display(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::Bool(b)) => if *b { "yes" } else { "no" }.to_string(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
    }
}
