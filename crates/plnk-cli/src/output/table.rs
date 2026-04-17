use comfy_table::{ContentArrangement, Table};
use plnk_core::error::PlankaError;
use plnk_core::models::Tabular;
use serde::Serialize;

use super::value_to_display;

/// Print a collection as a table to stdout.
#[allow(dead_code)] // Used once resource commands land
pub fn print_collection<T: Serialize + Tabular>(items: &[T]) -> Result<(), PlankaError> {
    if items.is_empty() {
        println!("No results.");
        return Ok(());
    }

    let columns = T::trimmed_columns();
    let headers: Vec<&str> = columns.iter().map(|(_, label)| *label).collect();

    let mut table = Table::new();
    table
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers);

    for item in items {
        let value = serde_json::to_value(item)?;
        let row: Vec<String> = columns
            .iter()
            .map(|(field, _)| value_to_display(value.get(*field)))
            .collect();
        table.add_row(row);
    }

    println!("{table}");
    Ok(())
}

/// Print a single item as a table to stdout.
pub fn print_item<T: Serialize + Tabular>(item: &T) -> Result<(), PlankaError> {
    let columns = T::trimmed_columns();
    let max_label = columns.iter().map(|(_, l)| l.len()).max().unwrap_or(0);
    let value = serde_json::to_value(item)?;

    for (field, label) in columns {
        let v = value_to_display(value.get(*field));
        println!("{label:>max_label$}: {v}");
    }
    Ok(())
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
    fn table_output_returns_error_when_serialize_fails() {
        let rows = [BrokenRow];
        let err = print_collection(&rows).unwrap_err();
        assert_eq!(err.error_type(), "SerializationError");
        assert!(err.to_string().contains("boom"));
    }
}
