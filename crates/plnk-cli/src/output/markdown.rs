use std::fmt::Write;

use plnk_core::models::Tabular;

/// Print a collection as a markdown table to stdout.
#[allow(dead_code)] // Used once resource commands land
pub fn print_collection<T: Tabular>(items: &[T]) {
    if items.is_empty() {
        println!("*No results.*");
        return;
    }

    let headers = T::headers();

    // Header row
    let mut header_line = String::new();
    for h in &headers {
        let _ = write!(header_line, "| {h} ");
    }
    header_line.push('|');
    println!("{header_line}");

    // Separator
    let mut sep_line = String::new();
    for h in &headers {
        let _ = write!(sep_line, "| {} ", "-".repeat(h.len()));
    }
    sep_line.push('|');
    println!("{sep_line}");

    // Data rows
    for item in items {
        let row = item.row();
        let mut row_line = String::new();
        for v in &row {
            let _ = write!(row_line, "| {v} ");
        }
        row_line.push('|');
        println!("{row_line}");
    }
}

/// Print a single item as markdown key-value pairs to stdout.
pub fn print_item<T: Tabular>(item: &T) {
    let headers = T::headers();
    let values = item.row();

    for (header, value) in headers.iter().zip(values.iter()) {
        println!("**{header}:** {value}");
    }
}
