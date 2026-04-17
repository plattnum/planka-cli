use comfy_table::{ContentArrangement, Table};
use plnk_core::models::Tabular;
use serde::Serialize;

use super::value_to_display;

/// Print a collection as a table to stdout.
#[allow(dead_code)] // Used once resource commands land
pub fn print_collection<T: Serialize + Tabular>(items: &[T]) {
    if items.is_empty() {
        println!("No results.");
        return;
    }

    let columns = T::trimmed_columns();
    let headers: Vec<&str> = columns.iter().map(|(_, label)| *label).collect();

    let mut table = Table::new();
    table
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers);

    for item in items {
        let value = serde_json::to_value(item).expect("serialize");
        let row: Vec<String> = columns
            .iter()
            .map(|(field, _)| value_to_display(value.get(*field)))
            .collect();
        table.add_row(row);
    }

    println!("{table}");
}

/// Print a single item as a table to stdout.
pub fn print_item<T: Serialize + Tabular>(item: &T) {
    let columns = T::trimmed_columns();
    let max_label = columns.iter().map(|(_, l)| l.len()).max().unwrap_or(0);
    let value = serde_json::to_value(item).expect("serialize");

    for (field, label) in columns {
        let v = value_to_display(value.get(*field));
        println!("{label:>max_label$}: {v}");
    }
}
