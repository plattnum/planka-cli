use std::fmt::Write;

use plnk_core::models::Tabular;
use serde::Serialize;

use super::value_to_display;

/// Print a collection as a markdown table to stdout.
#[allow(dead_code)] // Used once resource commands land
pub fn print_collection<T: Serialize + Tabular>(items: &[T]) {
    if items.is_empty() {
        println!("*No results.*");
        return;
    }

    let columns = T::trimmed_columns();

    // Header row
    let mut header_line = String::new();
    for (_, label) in columns {
        let _ = write!(header_line, "| {label} ");
    }
    header_line.push('|');
    println!("{header_line}");

    // Separator
    let mut sep_line = String::new();
    for (_, label) in columns {
        let _ = write!(sep_line, "| {} ", "-".repeat(label.len()));
    }
    sep_line.push('|');
    println!("{sep_line}");

    // Data rows
    for item in items {
        let value = serde_json::to_value(item).expect("serialize");
        let mut row_line = String::new();
        for (field, _) in columns {
            let v = value_to_display(value.get(*field));
            let _ = write!(row_line, "| {v} ");
        }
        row_line.push('|');
        println!("{row_line}");
    }
}

/// Print a single item as markdown key-value pairs to stdout.
pub fn print_item<T: Serialize + Tabular>(item: &T) {
    let columns = T::trimmed_columns();
    let value = serde_json::to_value(item).expect("serialize");

    for (field, label) in columns {
        let v = value_to_display(value.get(*field));
        println!("**{label}:** {v}");
    }
}
