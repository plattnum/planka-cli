use plnk_core::error::PlankaError;
use plnk_core::models::{Envelope, Meta, Tabular};
use serde::Serialize;

fn render_json_error(message: impl Into<String>) -> PlankaError {
    PlankaError::Json(serde_json::Error::io(std::io::Error::other(
        message.into(),
    )))
}

/// Print a collection as a full JSON envelope (all fields) to stdout.
pub fn print_collection_full<T: Serialize>(items: &[T]) -> Result<(), PlankaError> {
    let envelope = Envelope {
        success: true,
        data: items,
        meta: Some(Meta { count: items.len() }),
    };
    let json = serde_json::to_string_pretty(&envelope)?;
    println!("{json}");
    Ok(())
}

/// Print a single item as a full JSON envelope (all fields) to stdout.
pub fn print_item_full<T: Serialize>(item: &T) -> Result<(), PlankaError> {
    let envelope = Envelope {
        success: true,
        data: item,
        meta: None,
    };
    let json = serde_json::to_string_pretty(&envelope)?;
    println!("{json}");
    Ok(())
}

/// Print a collection as a trimmed JSON envelope to stdout.
///
/// Keys and types match the full envelope exactly — trimmed is a strict
/// projection. Only the fields listed in `Tabular::trimmed_columns()` survive.
pub fn print_collection_trimmed<T: Serialize + Tabular>(items: &[T]) -> Result<(), PlankaError> {
    let rows: Vec<serde_json::Value> = items
        .iter()
        .map(project::<T>)
        .collect::<Result<_, _>>()?;
    let envelope = serde_json::json!({
        "success": true,
        "data": rows,
        "meta": { "count": rows.len() }
    });
    let json = serde_json::to_string_pretty(&envelope)?;
    println!("{json}");
    Ok(())
}

/// Print a single item as a trimmed JSON envelope to stdout.
pub fn print_item_trimmed<T: Serialize + Tabular>(item: &T) -> Result<(), PlankaError> {
    let envelope = serde_json::json!({
        "success": true,
        "data": project::<T>(item)?
    });
    let json = serde_json::to_string_pretty(&envelope)?;
    println!("{json}");
    Ok(())
}

/// Project a serializable item to a JSON object containing only the
/// whitelisted fields declared by its `Tabular` impl. Types and nulls
/// from the original serialization are preserved.
fn project<T: Serialize + Tabular>(item: &T) -> Result<serde_json::Value, PlankaError> {
    let full = serde_json::to_value(item)?;
    let source = match full {
        serde_json::Value::Object(map) => map,
        other => {
            return Err(render_json_error(format!(
                "Tabular requires Serialize to produce a JSON object for {}, got {other:?}",
                std::any::type_name::<T>()
            )));
        }
    };

    let mut out = serde_json::Map::new();
    for (field, _label) in T::trimmed_columns() {
        let value = source.get(*field).cloned().ok_or_else(|| {
            render_json_error(format!(
                "Tabular::trimmed_columns field {field:?} missing from serialized {}",
                std::any::type_name::<T>()
            ))
        })?;
        out.insert((*field).to_string(), value);
    }
    Ok(serde_json::Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::ser::Error as _;
    use serde::{Serialize, Serializer};

    struct BrokenRow;

    impl Serialize for BrokenRow {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(S::Error::custom("boom"))
        }
    }

    impl Tabular for BrokenRow {
        fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
            &[("id", "ID")]
        }
    }

    #[test]
    fn trimmed_json_returns_error_when_serialize_fails() {
        let rows = [BrokenRow];
        let err = print_collection_trimmed(&rows).unwrap_err();
        assert_eq!(err.error_type(), "SerializationError");
        assert!(err.to_string().contains("boom"));
    }
}
