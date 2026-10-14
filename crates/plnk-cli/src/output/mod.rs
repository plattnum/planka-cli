mod json;
mod markdown;
mod table;

use plnk_core::error::PlankaError;
use plnk_core::models::Tabular;
use serde::Serialize;

use crate::app::OutputFormat;
use crate::names::NameIndex;

/// Render a collection of items to stdout.
pub fn render_collection<T: Serialize + Tabular>(
    items: &[T],
    format: OutputFormat,
    full: bool,
    names: Option<&NameIndex>,
) -> Result<(), PlankaError> {
    match format {
        OutputFormat::Json if full => json::print_collection_full(items, names),
        OutputFormat::Json => json::print_collection_trimmed(items, names),
        OutputFormat::Table => table::print_collection(items, names),
        OutputFormat::Markdown => markdown::print_collection(items, names),
    }
}

/// Render a collection with an explicit JSON `meta` payload.
pub fn render_collection_with_meta<T: Serialize + Tabular, M: Serialize>(
    items: &[T],
    format: OutputFormat,
    full: bool,
    meta: &M,
    names: Option<&NameIndex>,
) -> Result<(), PlankaError> {
    match format {
        OutputFormat::Json if full => json::print_collection_full_with_meta(items, meta, names),
        OutputFormat::Json => json::print_collection_trimmed_with_meta(items, meta, names),
        OutputFormat::Table => table::print_collection(items, names),
        OutputFormat::Markdown => markdown::print_collection(items, names),
    }
}

/// Render a single item to stdout.
pub fn render_item<T: Serialize + Tabular>(
    item: &T,
    format: OutputFormat,
    full: bool,
    names: Option<&NameIndex>,
) -> Result<(), PlankaError> {
    match format {
        OutputFormat::Json if full => json::print_item_full(item, names),
        OutputFormat::Json => json::print_item_trimmed(item, names),
        OutputFormat::Table => table::print_item(item, names),
        OutputFormat::Markdown => markdown::print_item(item, names),
    }
}

/// Prefetch names for a collection, then render it.
pub async fn render_collection_named<T, C>(
    client: &C,
    items: &[T],
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError>
where
    T: Serialize + Tabular,
    C: crate::names::NameSource + ?Sized,
{
    names.prefetch_items(client, items).await?;
    render_collection(items, format, full, Some(&*names))
}

/// Prefetch names for a collection with explicit JSON metadata, then render it.
pub async fn render_collection_with_meta_named<T, M, C>(
    client: &C,
    items: &[T],
    format: OutputFormat,
    full: bool,
    meta: &M,
    names: &mut NameIndex,
) -> Result<(), PlankaError>
where
    T: Serialize + Tabular,
    M: Serialize,
    C: crate::names::NameSource + ?Sized,
{
    names.prefetch_items(client, items).await?;
    render_collection_with_meta(items, format, full, meta, Some(&*names))
}

/// Prefetch names for a single item, then render it.
pub async fn render_item_named<T, C>(
    client: &C,
    item: &T,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError>
where
    T: Serialize + Tabular,
    C: crate::names::NameSource + ?Sized,
{
    names.prefetch_item(client, item).await?;
    render_item(item, format, full, Some(&*names))
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

/// Format one field for table/markdown display, substituting a resolved name
/// for a foreign-key value when an index is active.
pub(crate) fn field_display(
    value: &serde_json::Value,
    field: &str,
    names: Option<&NameIndex>,
) -> String {
    if let (Some(index), Some(id)) = (names, value.get(field).and_then(serde_json::Value::as_str)) {
        if let Some(name) = index.display(field, id) {
            return name.to_string();
        }
    }
    value_to_display(value.get(field))
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
