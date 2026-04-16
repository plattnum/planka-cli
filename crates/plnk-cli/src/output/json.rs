use plnk_core::models::{Envelope, Meta, Tabular};
use serde::Serialize;

/// Print a collection as a full JSON envelope (all fields) to stdout.
pub fn print_collection_full<T: Serialize>(items: &[T]) {
    let envelope = Envelope {
        success: true,
        data: items,
        meta: Some(Meta { count: items.len() }),
    };
    let json = serde_json::to_string_pretty(&envelope).expect("JSON serialization failed");
    println!("{json}");
}

/// Print a single item as a full JSON envelope (all fields) to stdout.
pub fn print_item_full<T: Serialize>(item: &T) {
    let envelope = Envelope {
        success: true,
        data: item,
        meta: None,
    };
    let json = serde_json::to_string_pretty(&envelope).expect("JSON serialization failed");
    println!("{json}");
}

/// Print a collection as a trimmed JSON envelope (Tabular fields only) to stdout.
pub fn print_collection_trimmed<T: Tabular>(items: &[T]) {
    let headers = T::headers();
    let rows: Vec<serde_json::Value> = items
        .iter()
        .map(|item| row_to_object(&headers, item))
        .collect();
    let envelope = serde_json::json!({
        "success": true,
        "data": rows,
        "meta": { "count": rows.len() }
    });
    let json = serde_json::to_string_pretty(&envelope).expect("JSON serialization failed");
    println!("{json}");
}

/// Print a single item as a trimmed JSON envelope (Tabular fields only) to stdout.
pub fn print_item_trimmed<T: Tabular>(item: &T) {
    let headers = T::headers();
    let obj = row_to_object(&headers, item);
    let envelope = serde_json::json!({
        "success": true,
        "data": obj
    });
    let json = serde_json::to_string_pretty(&envelope).expect("JSON serialization failed");
    println!("{json}");
}

/// Convert a Tabular row into a JSON object using headers as keys.
fn row_to_object<T: Tabular>(headers: &[&str], item: &T) -> serde_json::Value {
    let values = item.row();
    let mut map = serde_json::Map::new();
    for (header, value) in headers.iter().zip(values.iter()) {
        map.insert(
            header.to_lowercase(),
            serde_json::Value::String(value.clone()),
        );
    }
    serde_json::Value::Object(map)
}
