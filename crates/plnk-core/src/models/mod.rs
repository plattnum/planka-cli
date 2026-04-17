mod attachment;
mod board;
mod card;
mod comment;
mod common;
mod label;
mod list;
mod membership;
mod project;
mod task;
#[cfg(test)]
mod tests;
mod user;

pub use attachment::*;
pub use board::*;
pub use card::*;
pub use comment::*;
pub use common::*;
pub use label::*;
pub use list::*;
pub use membership::*;
pub use project::*;
pub use task::*;
pub use user::*;

/// Trait for types that declare a curated subset of fields for trimmed output.
///
/// Field names MUST match the Planka wire format exactly (serde camelCase).
/// Display labels are shown in table/markdown headers only — they never
/// leak into JSON output.
pub trait Tabular {
    /// `(serde_field_name, display_label)` pairs for trimmed output.
    fn trimmed_columns() -> &'static [(&'static str, &'static str)];
}

impl Tabular for Project {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[("id", "ID"), ("name", "Name")]
    }
}

impl Tabular for Board {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[
            ("id", "ID"),
            ("name", "Name"),
            ("projectId", "Project"),
            ("position", "Position"),
        ]
    }
}

impl Tabular for List {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[
            ("id", "ID"),
            ("name", "Name"),
            ("boardId", "Board"),
            ("position", "Position"),
        ]
    }
}

impl Tabular for Card {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[
            ("id", "ID"),
            ("name", "Name"),
            ("listId", "List"),
            ("position", "Position"),
            ("isClosed", "Closed"),
        ]
    }
}

impl Tabular for Task {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[("id", "ID"), ("name", "Name"), ("isCompleted", "Completed")]
    }
}

impl Tabular for Comment {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[
            ("id", "ID"),
            ("userId", "User"),
            ("text", "Text"),
            ("createdAt", "Created"),
        ]
    }
}

impl Tabular for Label {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[
            ("id", "ID"),
            ("name", "Name"),
            ("color", "Color"),
            ("boardId", "Board"),
        ]
    }
}

impl Tabular for User {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[
            ("id", "ID"),
            ("name", "Name"),
            ("username", "Username"),
            ("role", "Role"),
        ]
    }
}

impl Tabular for Attachment {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[
            ("id", "ID"),
            ("name", "Name"),
            ("cardId", "Card"),
            ("createdAt", "Created"),
        ]
    }
}

impl Tabular for BoardMembership {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[
            ("id", "ID"),
            ("userId", "User"),
            ("boardId", "Board"),
            ("role", "Role"),
        ]
    }
}

impl Tabular for ProjectManager {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[("id", "ID"), ("userId", "User"), ("projectId", "Project")]
    }
}

impl Tabular for CardMembership {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[("id", "ID"), ("userId", "User"), ("cardId", "Card")]
    }
}

impl Tabular for CardLabel {
    fn trimmed_columns() -> &'static [(&'static str, &'static str)] {
        &[("id", "ID"), ("cardId", "Card"), ("labelId", "Label")]
    }
}
