mod json;
mod markdown;
mod table;

use plnk_core::models::Tabular;
use serde::Serialize;

use crate::app::OutputFormat;

/// Render a collection of items to stdout.
#[allow(dead_code)] // Used once resource commands land
pub fn render_collection<T: Serialize + Tabular>(items: &[T], format: OutputFormat) {
    match format {
        OutputFormat::Json => json::print_collection(items),
        OutputFormat::Table => table::print_collection(items),
        OutputFormat::Markdown => markdown::print_collection(items),
    }
}

/// Render a single item to stdout.
pub fn render_item<T: Serialize + Tabular>(item: &T, format: OutputFormat) {
    match format {
        OutputFormat::Json => json::print_item(item),
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
pub fn render_message(message: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "success": true,
                    "data": { "message": message }
                })
            );
        }
        OutputFormat::Table | OutputFormat::Markdown => {
            println!("{message}");
        }
    }
}
