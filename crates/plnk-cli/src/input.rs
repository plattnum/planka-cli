use std::io::Read;

use plnk_core::error::PlankaError;

#[allow(dead_code)] // Used once resource commands with --description/--text land
/// Resolve a text value from the spec's input conventions:
/// - `"-"` reads from stdin
/// - `"@path"` reads from a file
/// - anything else is a literal string
pub fn resolve_text(value: &str) -> Result<String, PlankaError> {
    if value == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| PlankaError::FileReadError {
                path: "<stdin>".to_string(),
                source: e,
            })?;
        Ok(buf)
    } else if let Some(path) = value.strip_prefix('@') {
        std::fs::read_to_string(path).map_err(|e| PlankaError::FileReadError {
            path: path.to_string(),
            source: e,
        })
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_text() {
        let result = resolve_text("hello world").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn file_text() {
        // Use this source file as a known existing file
        let result = resolve_text("@Cargo.toml").unwrap();
        assert!(result.contains("[package]"));
    }

    #[test]
    fn file_not_found() {
        let result = resolve_text("@nonexistent_file_12345.txt");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_type(), "FileReadError");
    }

    #[test]
    fn empty_literal() {
        let result = resolve_text("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn at_sign_only_reads_empty_filename() {
        // "@" alone means file path "", which should fail
        let result = resolve_text("@");
        assert!(result.is_err());
    }
}
