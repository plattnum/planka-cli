use plnk_core::models::{Envelope, Meta};
use serde::Serialize;

/// Print a collection as a JSON envelope to stdout.
pub fn print_collection<T: Serialize>(items: &[T]) {
    let envelope = Envelope {
        success: true,
        data: items,
        meta: Some(Meta { count: items.len() }),
    };
    // Unwrap is safe: Serialize types don't fail serde_json serialization
    let json = serde_json::to_string_pretty(&envelope).expect("JSON serialization failed");
    println!("{json}");
}

/// Print a single item as a JSON envelope to stdout.
pub fn print_item<T: Serialize>(item: &T) {
    let envelope = Envelope {
        success: true,
        data: item,
        meta: None,
    };
    let json = serde_json::to_string_pretty(&envelope).expect("JSON serialization failed");
    println!("{json}");
}
