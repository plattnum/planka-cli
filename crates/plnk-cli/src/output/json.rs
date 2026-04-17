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

/// Print a collection as a trimmed JSON envelope to stdout.
///
/// Keys and types match the full envelope exactly — trimmed is a strict
/// projection. Only the fields listed in `Tabular::trimmed_columns()` survive.
pub fn print_collection_trimmed<T: Serialize + Tabular>(items: &[T]) {
    let rows: Vec<serde_json::Value> = items.iter().map(project::<T>).collect();
    let envelope = serde_json::json!({
        "success": true,
        "data": rows,
        "meta": { "count": rows.len() }
    });
    let json = serde_json::to_string_pretty(&envelope).expect("JSON serialization failed");
    println!("{json}");
}

/// Print a single item as a trimmed JSON envelope to stdout.
pub fn print_item_trimmed<T: Serialize + Tabular>(item: &T) {
    let envelope = serde_json::json!({
        "success": true,
        "data": project::<T>(item)
    });
    let json = serde_json::to_string_pretty(&envelope).expect("JSON serialization failed");
    println!("{json}");
}

/// Project a serializable item to a JSON object containing only the
/// whitelisted fields declared by its `Tabular` impl. Types and nulls
/// from the original serialization are preserved.
fn project<T: Serialize + Tabular>(item: &T) -> serde_json::Value {
    let full = serde_json::to_value(item).expect("JSON serialization failed");
    let source = match full {
        serde_json::Value::Object(map) => map,
        other => panic!(
            "Tabular requires Serialize to produce a JSON object for {}, got {other:?}",
            std::any::type_name::<T>()
        ),
    };
    let mut out = serde_json::Map::new();
    for (field, _label) in T::trimmed_columns() {
        let value = source.get(*field).cloned().unwrap_or_else(|| {
            panic!(
                "Tabular::trimmed_columns field {field:?} missing from serialized {}",
                std::any::type_name::<T>()
            )
        });
        out.insert((*field).to_string(), value);
    }
    serde_json::Value::Object(out)
}
