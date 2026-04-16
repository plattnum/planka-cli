//! Three-tier name matching for `find` operations.
//!
//! Matching stops at the first tier that produces results:
//! 1. Exact case-sensitive match
//! 2. Exact case-insensitive match
//! 3. Substring case-insensitive match

/// Trait for resources that have a searchable name.
pub trait Named {
    fn name(&self) -> &str;
}

/// Apply three-tier matching against a collection.
///
/// Returns references to all matching items from the first tier that
/// produces at least one result. If no tier matches, returns empty vec.
pub fn match_by_name<'a, T: Named>(items: &'a [T], query: &str) -> Vec<&'a T> {
    // Tier 1: exact case-sensitive
    let exact: Vec<_> = items.iter().filter(|i| i.name() == query).collect();
    if !exact.is_empty() {
        return exact;
    }

    // Tier 2: exact case-insensitive
    let lower = query.to_lowercase();
    let ci: Vec<_> = items
        .iter()
        .filter(|i| i.name().to_lowercase() == lower)
        .collect();
    if !ci.is_empty() {
        return ci;
    }

    // Tier 3: substring case-insensitive
    items
        .iter()
        .filter(|i| i.name().to_lowercase().contains(&lower))
        .collect()
}

// Implement Named for domain models that support find operations.

impl Named for crate::models::Board {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Named for crate::models::List {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Named for crate::models::Card {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Named for crate::models::Label {
    fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Item(String);
    impl Named for Item {
        fn name(&self) -> &str {
            &self.0
        }
    }

    fn items(names: &[&str]) -> Vec<Item> {
        names.iter().map(|n| Item(n.to_string())).collect()
    }

    #[test]
    fn tier1_exact_match() {
        let data = items(&["Sprint", "sprint", "My Sprint Board"]);
        let result = match_by_name(&data, "Sprint");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name(), "Sprint");
    }

    #[test]
    fn tier2_case_insensitive() {
        let data = items(&["sprint", "My Sprint Board"]);
        let result = match_by_name(&data, "Sprint");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name(), "sprint");
    }

    #[test]
    fn tier3_substring() {
        let data = items(&["My Sprint Board", "Another Board"]);
        let result = match_by_name(&data, "Sprint");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name(), "My Sprint Board");
    }

    #[test]
    fn no_match() {
        let data = items(&["Alpha", "Beta"]);
        let result = match_by_name(&data, "Gamma");
        assert!(result.is_empty());
    }

    #[test]
    fn tier1_stops_early() {
        // "Sprint" exists as exact match — should NOT also return "sprint" or substring
        let data = items(&["Sprint", "sprint", "Sprint Planning"]);
        let result = match_by_name(&data, "Sprint");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name(), "Sprint");
    }

    #[test]
    fn tier2_stops_early() {
        // No exact match, but case-insensitive matches exist — should NOT fall to substring
        let data = items(&["sprint", "SPRINT", "Sprint Planning"]);
        let result = match_by_name(&data, "Sprint");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn multiple_substring_matches() {
        let data = items(&["Fix auth bug", "Fix auth race", "Other"]);
        let result = match_by_name(&data, "auth");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn empty_query() {
        let data = items(&["Alpha", "Beta"]);
        let result = match_by_name(&data, "");
        // Empty string is a substring of everything
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn empty_collection() {
        let data: Vec<Item> = vec![];
        let result = match_by_name(&data, "Sprint");
        assert!(result.is_empty());
    }
}
