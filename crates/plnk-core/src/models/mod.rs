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

/// Trait for types that can be rendered as table rows.
pub trait Tabular {
    fn headers() -> Vec<&'static str>;
    fn row(&self) -> Vec<String>;
}

impl Tabular for Project {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Name"]
    }

    fn row(&self) -> Vec<String> {
        vec![self.id.clone(), self.name.clone()]
    }
}

impl Tabular for Board {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Name", "Project", "Position"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.project_id.clone(),
            self.position.to_string(),
        ]
    }
}

impl Tabular for List {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Name", "Board", "Position"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.board_id.clone(),
            self.position.to_string(),
        ]
    }
}

impl Tabular for Card {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Name", "List", "Position", "Closed"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.list_id.clone(),
            self.position.to_string(),
            if self.is_closed {
                "yes".to_string()
            } else {
                "no".to_string()
            },
        ]
    }
}

impl Tabular for Task {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Name", "Completed"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            if self.is_completed {
                "yes".to_string()
            } else {
                "no".to_string()
            },
        ]
    }
}

impl Tabular for Comment {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "User", "Text", "Created"]
    }

    fn row(&self) -> Vec<String> {
        let text_preview = if self.text.len() > 60 {
            // Find a char-safe truncation point to avoid panicking on multi-byte UTF-8
            let end = self
                .text
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= 57)
                .last()
                .unwrap_or(0);
            format!("{}...", &self.text[..end])
        } else {
            self.text.clone()
        };
        vec![
            self.id.clone(),
            self.user_id.clone(),
            text_preview,
            self.created_at.clone(),
        ]
    }
}

impl Tabular for Label {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Name", "Color", "Board"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone().unwrap_or_default(),
            self.color.clone(),
            self.board_id.clone(),
        ]
    }
}

impl Tabular for User {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Name", "Username", "Role"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.username.clone().unwrap_or_default(),
            self.role.clone(),
        ]
    }
}

impl Tabular for Attachment {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Name", "Card", "Created"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.card_id.clone(),
            self.created_at.clone(),
        ]
    }
}

impl Tabular for BoardMembership {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "User", "Board", "Role"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.user_id.clone(),
            self.board_id.clone(),
            self.role.clone().unwrap_or_default(),
        ]
    }
}

impl Tabular for ProjectManager {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "User", "Project"]
    }

    fn row(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.user_id.clone(),
            self.project_id.clone(),
        ]
    }
}

impl Tabular for CardMembership {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "User", "Card"]
    }

    fn row(&self) -> Vec<String> {
        vec![self.id.clone(), self.user_id.clone(), self.card_id.clone()]
    }
}

impl Tabular for CardLabel {
    fn headers() -> Vec<&'static str> {
        vec!["ID", "Card", "Label"]
    }

    fn row(&self) -> Vec<String> {
        vec![self.id.clone(), self.card_id.clone(), self.label_id.clone()]
    }
}
