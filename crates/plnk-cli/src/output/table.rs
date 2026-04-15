use comfy_table::{ContentArrangement, Table};
use plnk_core::models::Tabular;

/// Print a collection as a table to stdout.
#[allow(dead_code)] // Used once resource commands land
pub fn print_collection<T: Tabular>(items: &[T]) {
    if items.is_empty() {
        println!("No results.");
        return;
    }

    let mut table = Table::new();
    table
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(T::headers());

    for item in items {
        table.add_row(item.row());
    }

    println!("{table}");
}

/// Print a single item as a table to stdout.
pub fn print_item<T: Tabular>(item: &T) {
    let headers = T::headers();
    let values = item.row();

    let max_label = headers.iter().map(|h| h.len()).max().unwrap_or(0);

    for (header, value) in headers.iter().zip(values.iter()) {
        println!("{header:>max_label$}: {value}");
    }
}
