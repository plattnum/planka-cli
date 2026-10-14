use plnk_core::api::match_by_name;
use plnk_core::error::PlankaError;
use plnk_core::models::UpdateCustomFieldGroup;

use crate::app::OutputFormat;
use crate::commands::project::confirm_delete;
use crate::names::{NameIndex, NameSource};
use crate::output::{render_collection_named, render_item_named, render_message};

/// Base groups and ordinary groups are separate wire resources reached by
/// different routes, and an ID alone does not say which kind it is. `get`,
/// `update` and `delete` therefore try the ordinary route first and fall back
/// to the base route on a not-found, so a caller can pass any group ID that
/// `create` handed them without tracking its kind.
fn is_not_found(error: &PlankaError) -> bool {
    matches!(
        error,
        PlankaError::Remote404 { .. } | PlankaError::NotFound { .. }
    )
}

/// The `--project` / `--board` / `--card` scope shared by `list`, `find` and
/// `create`. Clap guarantees at most one is set; exactly-one is checked here.
struct Scope {
    project: Option<String>,
    board: Option<String>,
    card: Option<String>,
}

fn missing_scope() -> PlankaError {
    PlankaError::MissingRequiredOption {
        field: "--project, --board, or --card".to_string(),
    }
}

/// A project yields base groups; a board or card yields ordinary groups. These
/// are different types with different columns and are deliberately not forced
/// through one shape.
async fn list(
    client: &impl NameSource,
    scope: Scope,
    name: Option<&str>,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    if let Some(project_id) = scope.project {
        let groups = client.list_base_field_groups(&project_id).await?;
        let groups = match name {
            Some(query) => match_by_name(&groups, query).into_iter().cloned().collect(),
            None => groups,
        };
        render_collection_named(client, &groups, format, full, names).await
    } else if let Some(board_id) = scope.board {
        let groups = client.list_field_groups_for_board(&board_id).await?;
        let groups = match name {
            Some(query) => match_by_name(&groups, query).into_iter().cloned().collect(),
            None => groups,
        };
        render_collection_named(client, &groups, format, full, names).await
    } else if let Some(card_id) = scope.card {
        let groups = client.list_field_groups_for_card(&card_id).await?;
        let groups = match name {
            Some(query) => match_by_name(&groups, query).into_iter().cloned().collect(),
            None => groups,
        };
        render_collection_named(client, &groups, format, full, names).await
    } else {
        Err(missing_scope())
    }
}

async fn create(
    client: &impl NameSource,
    scope: Scope,
    name: Option<String>,
    base: Option<String>,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    let require_name = |name: Option<String>| {
        name.ok_or_else(|| PlankaError::MissingRequiredOption {
            field: "--name".to_string(),
        })
    };

    if let Some(project_id) = scope.project {
        let name = require_name(name)?;
        let group = client.create_base_field_group(&project_id, &name).await?;
        render_item_named(client, &group, format, full, names).await
    } else if let Some(board_id) = scope.board {
        let name = require_name(name)?;
        let group = client.create_board_field_group(&board_id, &name).await?;
        render_item_named(client, &group, format, full, names).await
    } else if let Some(card_id) = scope.card {
        if base.is_none() && name.is_none() {
            return Err(PlankaError::InvalidOptionValue {
                field: "--base / --name".to_string(),
                message: "Pass --base <baseGroupId> to adopt a template, or --name <name> to \
                          create a one-off group"
                    .to_string(),
            });
        }
        let base = match base {
            Some(base) => Some(names.resolve_base_group(client, &base).await?),
            None => None,
        };
        let group = client
            .create_card_field_group(&card_id, base.as_deref(), name.as_deref())
            .await?;
        render_item_named(client, &group, format, full, names).await
    } else {
        Err(missing_scope())
    }
}

async fn get(
    client: &impl NameSource,
    id: &str,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    let id = names.resolve_group(client, id).await?;
    match client.get_field_group(&id).await {
        Ok(group) => render_item_named(client, &group, format, full, names).await,
        Err(error) if is_not_found(&error) => {
            let base = client.get_base_field_group(&id).await.map_err(|_| error)?;
            render_item_named(client, &base, format, full, names).await
        }
        Err(error) => Err(error),
    }
}

async fn update(
    client: &impl NameSource,
    id: &str,
    name: Option<String>,
    format: OutputFormat,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    if name.is_none() {
        return Err(PlankaError::InvalidOptionValue {
            field: "--name".to_string(),
            message: "At least one field must be provided for update".to_string(),
        });
    }
    let id = names.resolve_group(client, id).await?;
    let params = UpdateCustomFieldGroup { name };
    match client.update_field_group(&id, params.clone()).await {
        Ok(group) => render_item_named(client, &group, format, full, names).await,
        Err(error) if is_not_found(&error) => {
            let base = client
                .update_base_field_group(&id, params)
                .await
                .map_err(|_| error)?;
            render_item_named(client, &base, format, full, names).await
        }
        Err(error) => Err(error),
    }
}

async fn delete(
    client: &impl NameSource,
    id: &str,
    format: OutputFormat,
    yes: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    let id = names.resolve_group(client, id).await?;
    if !yes && !confirm_delete("field group", &id) {
        return render_message("Aborted.", format);
    }
    match client.delete_field_group(&id).await {
        Ok(()) => {}
        Err(error) if is_not_found(&error) => {
            // The ordinary route made no change, so retrying the base route is safe.
            client
                .delete_base_field_group(&id)
                .await
                .map_err(|_| error)?;
        }
        Err(error) => return Err(error),
    }
    render_message("Field group deleted.", format)
}

pub async fn execute(
    client: &impl NameSource,
    action: crate::app::FieldGroupAction,
    format: OutputFormat,
    yes: bool,
    full: bool,
    names: &mut NameIndex,
) -> Result<(), PlankaError> {
    use crate::app::FieldGroupAction as Action;

    match action {
        Action::List {
            project,
            board,
            card,
        } => {
            let project = names.resolve_optional_project(client, project).await?;
            let board = names
                .resolve_optional_board(client, board, project.as_deref())
                .await?;
            let scope = Scope {
                project,
                board,
                card,
            };
            list(client, scope, None, format, full, names).await
        }
        Action::Find {
            project,
            board,
            card,
            name,
        } => {
            let project = names.resolve_optional_project(client, project).await?;
            let board = names
                .resolve_optional_board(client, board, project.as_deref())
                .await?;
            let scope = Scope {
                project,
                board,
                card,
            };
            list(client, scope, Some(&name), format, full, names).await
        }
        Action::Get { id } => get(client, &id, format, full, names).await,
        Action::Create {
            project,
            board,
            card,
            name,
            base,
        } => {
            let project = names.resolve_optional_project(client, project).await?;
            let board = names
                .resolve_optional_board(client, board, project.as_deref())
                .await?;
            let scope = Scope {
                project,
                board,
                card,
            };
            create(client, scope, name, base, format, full, names).await
        }
        Action::Update { id, name } => update(client, &id, name, format, full, names).await,
        Action::Delete { id } => delete(client, &id, format, yes, names).await,
    }
}
