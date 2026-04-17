use std::collections::BTreeMap;

use clap::CommandFactory;
use serde::Serialize;

use crate::app::App;

/// Machine-readable help per spec section 11.
#[derive(Debug, Serialize)]
pub struct CommandHelp {
    pub resource: String,
    pub action: String,
    pub summary: String,
    pub args: Vec<ArgHelp>,
    pub options: BTreeMap<String, OptionHelp>,
    pub examples: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ArgHelp {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct OptionHelp {
    #[serde(rename = "type")]
    pub opt_type: String,
    pub required: bool,
    pub description: String,
}

/// Check if raw CLI args contain both `--help`/`-h` and `--output json`.
/// If so, render machine-readable help JSON to stdout and return `true`.
pub fn try_machine_help() -> bool {
    let raw_args: Vec<String> = std::env::args().collect();

    let has_help = raw_args.iter().any(|a| a == "--help" || a == "-h");
    let has_json_output = raw_args
        .windows(2)
        .any(|w| w[0] == "--output" && w[1] == "json");

    if !has_help || !has_json_output {
        return false;
    }

    let positionals = extract_positionals(&raw_args);
    let cmd = App::command();

    let help = build_help(&cmd, &positionals);
    let json = serde_json::to_string_pretty(&help).expect("JSON serialization failed");
    println!("{json}");
    true
}

/// Extract positional arguments from raw CLI args, skipping flags and their values.
fn extract_positionals(raw_args: &[String]) -> Vec<String> {
    let value_flags: &[&str] = &["--server", "--token", "--output"];
    let mut skip_next = false;
    let mut positionals = Vec::new();

    for arg in raw_args.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg.starts_with('-') {
            if value_flags.contains(&arg.as_str()) {
                skip_next = true;
            }
            continue;
        }
        positionals.push(arg.clone());
    }

    positionals
}

/// Navigate the clap command tree using positionals and build help.
fn build_help(root: &clap::Command, positionals: &[String]) -> CommandHelp {
    if positionals.is_empty() {
        return build_top_level_help(root);
    }

    // Walk down the command tree following subcommands
    let mut current = root;
    let mut path = Vec::new();

    for arg in positionals {
        if let Some(sub) = current.find_subcommand(arg) {
            path.push(arg.as_str());
            current = sub;
        } else {
            // Remaining positionals are arg values, not subcommands
            break;
        }
    }

    match path.len() {
        0 => build_top_level_help(root),
        1 => build_resource_help(path[0], current),
        _ => build_action_help(&path, current),
    }
}

/// Top-level: list all resources.
fn build_top_level_help(cmd: &clap::Command) -> CommandHelp {
    let mut options = BTreeMap::new();
    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() || sub.get_name() == "help" {
            continue;
        }
        let about = sub
            .get_about()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        options.insert(
            sub.get_name().to_string(),
            OptionHelp {
                opt_type: "resource".to_string(),
                required: false,
                description: about,
            },
        );
    }

    CommandHelp {
        resource: String::new(),
        action: String::new(),
        summary: cmd
            .get_about()
            .map(std::string::ToString::to_string)
            .unwrap_or_default(),
        args: Vec::new(),
        options,
        examples: Vec::new(),
    }
}

/// Resource-level: list available actions for a resource.
fn build_resource_help(resource: &str, cmd: &clap::Command) -> CommandHelp {
    let mut options = BTreeMap::new();
    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() || sub.get_name() == "help" {
            continue;
        }
        let about = sub
            .get_about()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        options.insert(
            sub.get_name().to_string(),
            OptionHelp {
                opt_type: "action".to_string(),
                required: false,
                description: about,
            },
        );
    }

    CommandHelp {
        resource: resource.to_string(),
        action: String::new(),
        summary: cmd
            .get_about()
            .map(std::string::ToString::to_string)
            .unwrap_or_default(),
        args: Vec::new(),
        options,
        examples: Vec::new(),
    }
}

/// Action-level: full help for a specific resource action.
fn build_action_help(path: &[&str], cmd: &clap::Command) -> CommandHelp {
    let resource = path[0];
    let action = path[1..].join(" ");

    let summary = cmd
        .get_about()
        .map(std::string::ToString::to_string)
        .unwrap_or_default();

    let mut args = Vec::new();
    let mut options = BTreeMap::new();

    for arg in cmd.get_arguments() {
        let name = arg.get_id().as_str();

        if is_global_flag(name) {
            continue;
        }

        let help = arg
            .get_help()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        let required = arg.is_required_set();
        let arg_type = infer_type(name);

        if arg.is_positional() {
            args.push(ArgHelp {
                name: name.to_string(),
                arg_type,
                required,
                description: help,
            });
        } else {
            let flag_name = format!("--{name}");
            options.insert(
                flag_name,
                OptionHelp {
                    opt_type: arg_type,
                    required,
                    description: help,
                },
            );
        }
    }

    // If the action command has subcommands, list them (e.g., card > label, card > assignee)
    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() || sub.get_name() == "help" {
            continue;
        }
        let about = sub
            .get_about()
            .map(std::string::ToString::to_string)
            .unwrap_or_default();
        options.insert(
            sub.get_name().to_string(),
            OptionHelp {
                opt_type: "subcommand".to_string(),
                required: false,
                description: about,
            },
        );
    }

    let examples = get_examples(resource, &action);

    CommandHelp {
        resource: resource.to_string(),
        action,
        summary,
        args,
        options,
        examples,
    }
}

fn is_global_flag(name: &str) -> bool {
    matches!(
        name,
        "server"
            | "token"
            | "output"
            | "verbose"
            | "quiet"
            | "no_color"
            | "no-color"
            | "yes"
            | "full"
            | "help"
            | "version"
    )
}

/// Infer a user-facing type string from the argument name.
fn infer_type(name: &str) -> String {
    match name {
        "description" | "text" => "text".to_string(),
        "position" => "enum(top|bottom|int)".to_string(),
        "role" => "enum(admin|editor|viewer)".to_string(),
        "file" | "out" => "path".to_string(),
        _ => "string".to_string(),
    }
}

/// Hand-maintained examples for common resource/action combos.
#[allow(clippy::too_many_lines)]
fn get_examples(resource: &str, action: &str) -> Vec<String> {
    match (resource, action) {
        // Auth
        ("auth", "login") => vec![
            "plnk auth login --server https://planka.example.com".into(),
            "plnk auth login --server https://planka.example.com --email user@example.com --password secret".into(),
        ],
        ("auth", "whoami") => vec!["plnk auth whoami".into()],
        ("auth", "status") => vec!["plnk auth status".into()],
        ("auth", "logout") => vec!["plnk auth logout".into()],

        // Project
        ("project", "list") => vec!["plnk project list".into()],
        ("project", "get") => vec!["plnk project get 123".into()],
        ("project", "create") => vec!["plnk project create --name 'Platform'".into()],
        ("project", "find") => vec!["plnk project find --name 'Platform'".into()],
        ("project", "snapshot") => vec!["plnk project snapshot 123 --output json".into()],
        ("project", "update") => vec!["plnk project update 123 --name 'Platform Core'".into()],
        ("project", "delete") => vec!["plnk project delete 123".into()],

        // User
        ("user", "list") => vec!["plnk user list".into()],
        ("user", "get") => vec!["plnk user get 88".into()],

        // Board
        ("board", "list") => vec!["plnk board list --project 123".into()],
        ("board", "get") => vec!["plnk board get 456".into()],
        ("board", "find") => vec!["plnk board find --project 123 --name 'Sprint'".into()],
        ("board", "snapshot") => vec!["plnk board snapshot 456 --output json".into()],
        ("board", "create") => vec!["plnk board create --project 123 --name 'Sprint'".into()],
        ("board", "update") => vec!["plnk board update 456 --name 'Sprint 2'".into()],
        ("board", "delete") => vec!["plnk board delete 456".into()],

        // List
        ("list", "list") => vec!["plnk list list --board 456".into()],
        ("list", "get") => vec!["plnk list get 789".into()],
        ("list", "find") => vec!["plnk list find --board 456 --name 'Backlog'".into()],
        ("list", "create") => vec!["plnk list create --board 456 --name 'Doing'".into()],
        ("list", "move") => vec!["plnk list move 789 --to-position 2".into()],
        ("list", "delete") => vec!["plnk list delete 789".into()],

        // Card
        ("card", "list") => vec!["plnk card list --list 789".into()],
        ("card", "get") => vec!["plnk card get 1234".into()],
        ("card", "snapshot") => vec!["plnk card snapshot 1234 --output json".into()],
        ("card", "find") => vec![
            "plnk card find --list 789 --title 'Fix auth'".into(),
            "plnk card find --board 456 --title 'Fix auth'".into(),
            "plnk card find --project 123 --title 'Fix auth'".into(),
        ],
        ("card", "create") => vec![
            "plnk card create --list 789 --title 'Fix auth'".into(),
            "plnk card create --list 789 --title 'Fix auth' --description @spec.md".into(),
            "plnk card create --list 789 --title 'Fix auth' --position top".into(),
        ],
        ("card", "update") => vec![
            "plnk card update 1234 --title 'Fix auth race'".into(),
            "plnk card update 1234 --description @spec.md".into(),
        ],
        ("card", "move") => vec!["plnk card move 1234 --to-list 790 --position top".into()],
        ("card", "archive") => vec!["plnk card archive 1234".into()],
        ("card", "unarchive") => vec!["plnk card unarchive 1234".into()],
        ("card", "delete") => vec!["plnk card delete 1234".into()],

        // Card label
        ("card", "label list") => vec!["plnk card label list 1234".into()],
        ("card", "label add") => vec!["plnk card label add 1234 111".into()],
        ("card", "label remove") => vec!["plnk card label remove 1234 111".into()],

        // Card assignee
        ("card", "assignee list") => vec!["plnk card assignee list 1234".into()],
        ("card", "assignee add") => vec!["plnk card assignee add 1234 88".into()],
        ("card", "assignee remove") => vec!["plnk card assignee remove 1234 88".into()],

        // Task
        ("task", "list") => vec!["plnk task list --card 1234".into()],
        ("task", "get") => vec!["plnk task get 5678".into()],
        ("task", "create") => vec!["plnk task create --card 1234 --title 'Write tests'".into()],
        ("task", "complete") => vec!["plnk task complete 5678".into()],
        ("task", "reopen") => vec!["plnk task reopen 5678".into()],
        ("task", "delete") => vec!["plnk task delete 5678".into()],

        // Comment
        ("comment", "list") => vec!["plnk comment list --card 1234".into()],
        ("comment", "get") => vec!["plnk comment get 9012".into()],
        ("comment", "create") => vec![
            "plnk comment create --card 1234 --text 'Starting work'".into(),
            "plnk comment create --card 1234 --text @note.txt".into(),
        ],
        ("comment", "update") => vec!["plnk comment update 9012 --text 'Blocked on API'".into()],
        ("comment", "delete") => vec!["plnk comment delete 9012".into()],

        // Label
        ("label", "list") => vec!["plnk label list --board 456".into()],
        ("label", "get") => vec!["plnk label get 111".into()],
        ("label", "find") => vec!["plnk label find --board 456 --name 'urgent'".into()],
        ("label", "create") => vec!["plnk label create --board 456 --name 'urgent' --color berry-red".into()],
        ("label", "update") => vec!["plnk label update 111 --name 'blocked' --color sunset-orange".into()],
        ("label", "delete") => vec!["plnk label delete 111".into()],

        // Attachment
        ("attachment", "list") => vec!["plnk attachment list --card 1234".into()],
        ("attachment", "upload") => vec!["plnk attachment upload --card 1234 ./spec.png".into()],
        ("attachment", "download") => vec![
            "plnk attachment download 555 --card 1234".into(),
            "plnk attachment download 555 --card 1234 --out ./renamed.png".into(),
        ],
        ("attachment", "delete") => vec!["plnk attachment delete 555".into()],

        // Membership
        ("membership", "list") => vec![
            "plnk membership list --project 123".into(),
            "plnk membership list --board 456".into(),
        ],
        ("membership", "add") => vec!["plnk membership add --project 123 --user 88 --role editor".into()],
        ("membership", "remove") => vec!["plnk membership remove --board 456 --user 88".into()],

        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_positionals_basic() {
        let args: Vec<String> = vec![
            "plnk".into(),
            "card".into(),
            "create".into(),
            "--help".into(),
            "--output".into(),
            "json".into(),
        ];
        let pos = extract_positionals(&args);
        assert_eq!(pos, vec!["card", "create"]);
    }

    #[test]
    fn extract_positionals_with_value_flags() {
        let args: Vec<String> = vec![
            "plnk".into(),
            "--server".into(),
            "http://example.com".into(),
            "board".into(),
            "list".into(),
            "--output".into(),
            "json".into(),
            "--help".into(),
        ];
        let pos = extract_positionals(&args);
        assert_eq!(pos, vec!["board", "list"]);
    }

    #[test]
    fn extract_positionals_with_positional_id() {
        let args: Vec<String> = vec![
            "plnk".into(),
            "card".into(),
            "get".into(),
            "1234".into(),
            "--help".into(),
            "--output".into(),
            "json".into(),
        ];
        let pos = extract_positionals(&args);
        assert_eq!(pos, vec!["card", "get", "1234"]);
    }

    #[test]
    fn build_help_card_create() {
        let cmd = App::command();
        let help = build_help(&cmd, &["card".into(), "create".into()]);
        assert_eq!(help.resource, "card");
        assert_eq!(help.action, "create");
        assert!(!help.summary.is_empty());
        assert!(help.options.contains_key("--list"));
        assert!(help.options.contains_key("--title"));
        assert!(help.options["--list"].required);
        assert!(help.options["--title"].required);
        assert!(!help.examples.is_empty());
    }

    #[test]
    fn build_help_resource_level() {
        let cmd = App::command();
        let help = build_help(&cmd, &["card".into()]);
        assert_eq!(help.resource, "card");
        assert_eq!(help.action, "");
        assert!(help.options.contains_key("list"));
        assert!(help.options.contains_key("get"));
        assert!(help.options.contains_key("create"));
    }

    #[test]
    fn build_help_top_level() {
        let cmd = App::command();
        let help = build_help(&cmd, &[]);
        assert_eq!(help.resource, "");
        assert!(help.options.contains_key("card"));
        assert!(help.options.contains_key("project"));
        // Aliases should be hidden
        assert!(!help.options.contains_key("boards"));
        assert!(!help.options.contains_key("cards"));
    }

    #[test]
    fn build_help_nested_subcommand() {
        let cmd = App::command();
        let help = build_help(&cmd, &["card".into(), "label".into(), "add".into()]);
        assert_eq!(help.resource, "card");
        assert_eq!(help.action, "label add");
    }

    #[test]
    fn option_types_inferred() {
        assert_eq!(infer_type("description"), "text");
        assert_eq!(infer_type("text"), "text");
        assert_eq!(infer_type("position"), "enum(top|bottom|int)");
        assert_eq!(infer_type("title"), "string");
        assert_eq!(infer_type("file"), "path");
    }

    #[test]
    fn global_flags_excluded() {
        let cmd = App::command();
        let help = build_help(&cmd, &["card".into(), "create".into()]);
        assert!(!help.options.contains_key("--server"));
        assert!(!help.options.contains_key("--token"));
        assert!(!help.options.contains_key("--output"));
        assert!(!help.options.contains_key("--verbose"));
        assert!(!help.options.contains_key("--quiet"));
    }
}
