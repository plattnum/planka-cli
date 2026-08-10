use plnk_core::api::{CardCustomFieldApi, CustomFieldApi, CustomFieldGroupApi, match_by_name};
use plnk_core::error::PlankaError;
use plnk_core::models::{
    BaseCustomFieldGroup, CUSTOM_FIELD_VALUE_MAX_LEN, CustomField, CustomFieldGroup,
};

use crate::app::OutputFormat;
use crate::output::{render_collection, render_item, render_message};

/// A card's group paired with the display name it actually presents.
///
/// A group adopted from a base group has `name: None` — its display name lives
/// on the base group, reached through `base_custom_field_group_id`. Matching on
/// a card group's own `name` alone therefore matches *nothing* for every
/// template-adopted group, which is the common case.
struct NamedGroup {
    group: CustomFieldGroup,
    display_name: String,
}

impl plnk_core::api::Named for NamedGroup {
    fn name(&self) -> &str {
        &self.display_name
    }
}

fn ambiguous(flag: &str, query: &str, candidates: &[(String, String)]) -> PlankaError {
    let listed = candidates
        .iter()
        .map(|(name, id)| {
            let shown = if name.is_empty() { "<unnamed>" } else { name };
            format!("{shown} ({id})")
        })
        .collect::<Vec<_>>()
        .join(", ");
    PlankaError::InvalidOptionValue {
        field: flag.to_string(),
        message: format!(
            "'{query}' matched multiple entries. Be more specific or pass an ID. \
             Matches: {listed}"
        ),
    }
}

/// Pair each of the card's groups with its effective display name, falling back
/// through the base group when the card group's own name is null.
async fn named_groups(
    client: &impl CustomFieldGroupApi,
    card_id: &str,
) -> Result<Vec<NamedGroup>, PlankaError> {
    let groups = client.list_field_groups_for_card(card_id).await?;

    // Only pay for the base-group lookup when some group actually needs it.
    let needs_base = groups
        .iter()
        .any(|group| group.name.is_none() && group.base_custom_field_group_id.is_some());
    let base_groups: Vec<BaseCustomFieldGroup> = if needs_base {
        client.list_all_base_field_groups().await?
    } else {
        Vec::new()
    };

    Ok(groups
        .into_iter()
        .map(|group| {
            let display_name = group.name.clone().unwrap_or_else(|| {
                group
                    .base_custom_field_group_id
                    .as_ref()
                    .and_then(|base_id| {
                        base_groups
                            .iter()
                            .find(|base| base.id == *base_id)
                            .map(|base| base.name.clone())
                    })
                    .unwrap_or_default()
            });
            NamedGroup {
                group,
                display_name,
            }
        })
        .collect())
}

/// Resolve `--group` to a card group. An argument that is already one of the
/// card's group IDs skips resolution entirely.
async fn resolve_group(
    client: &impl CustomFieldGroupApi,
    card_id: &str,
    query: &str,
) -> Result<CustomFieldGroup, PlankaError> {
    let candidates = named_groups(client, card_id).await?;

    if let Some(found) = candidates.iter().find(|entry| entry.group.id == query) {
        return Ok(found.group.clone());
    }

    let matched = match_by_name(&candidates, query);
    match matched.len() {
        0 => Err(PlankaError::NotFoundMessage {
            message: format!(
                "No custom field group matching '{query}' is attached to this card. \
                 Use 'plnk field-group list --card {card_id}' to inspect the card's \
                 groups or pass a group ID."
            ),
        }),
        1 => Ok(matched[0].group.clone()),
        _ => {
            let listed = matched
                .iter()
                .map(|entry| (entry.display_name.clone(), entry.group.id.clone()))
                .collect::<Vec<_>>();
            Err(ambiguous("--group", query, &listed))
        }
    }
}

/// Every field reachable through a group: its own fields plus, for an adopted
/// group, the base group's fields — which is where an adopted group's fields
/// actually live.
async fn fields_for_group(
    client: &impl CustomFieldApi,
    group: &CustomFieldGroup,
) -> Result<Vec<CustomField>, PlankaError> {
    let mut fields = client.list_fields(&group.id, false).await?;

    if let Some(base_id) = &group.base_custom_field_group_id {
        for field in client.list_fields(base_id, true).await? {
            if !fields.iter().any(|existing| existing.id == field.id) {
                fields.push(field);
            }
        }
    }

    Ok(fields)
}

async fn resolve_field(
    client: &impl CustomFieldApi,
    group: &CustomFieldGroup,
    query: &str,
) -> Result<String, PlankaError> {
    let fields = fields_for_group(client, group).await?;

    if fields.iter().any(|field| field.id == query) {
        return Ok(query.to_string());
    }

    let matched = match_by_name(&fields, query);
    match matched.len() {
        0 => Err(PlankaError::NotFoundMessage {
            message: format!(
                "No custom field matching '{query}' was found in this group. \
                 Use 'plnk field list --group {}' to inspect its fields or pass \
                 a field ID.",
                group.id
            ),
        }),
        1 => Ok(matched[0].id.clone()),
        _ => {
            let listed = matched
                .iter()
                .map(|field| (field.name.clone(), field.id.clone()))
                .collect::<Vec<_>>();
            Err(ambiguous("--field", query, &listed))
        }
    }
}

/// Validate a value before any request is issued.
///
/// The server rejects both cases, but a client-side check turns a wasted round
/// trip and an opaque `400` into a validation error with a clear message.
fn validate_value(value: &str) -> Result<(), PlankaError> {
    if value.is_empty() {
        return Err(PlankaError::InvalidOptionValue {
            field: "--value".to_string(),
            message: "A custom field value cannot be empty. Use \
                      'plnk card field clear' to remove a value."
                .to_string(),
        });
    }

    // Planka counts characters, not bytes. A string of astral-plane characters
    // may still be rejected server-side, since JavaScript measures UTF-16 code
    // units; that degrades to the server's own 400 rather than a wrong accept.
    let length = value.chars().count();
    if length > CUSTOM_FIELD_VALUE_MAX_LEN {
        return Err(PlankaError::InvalidOptionValue {
            field: "--value".to_string(),
            message: format!(
                "A custom field value is capped at {CUSTOM_FIELD_VALUE_MAX_LEN} characters, \
                 got {length}."
            ),
        });
    }

    Ok(())
}

pub async fn execute(
    client: &(impl CardCustomFieldApi + CustomFieldGroupApi + CustomFieldApi),
    action: crate::app::CardFieldAction,
    format: OutputFormat,
    full: bool,
) -> Result<(), PlankaError> {
    use crate::app::CardFieldAction as Action;

    match action {
        Action::List { card } => {
            let values = client.list_field_values(&card).await?;
            render_collection(&values, format, full)?;
        }
        Action::Set {
            card,
            group,
            field,
            value,
        } => {
            // Validate before resolving so a bad value costs no requests at all.
            validate_value(&value)?;
            let group = resolve_group(client, &card, &group).await?;
            let field_id = resolve_field(client, &group, &field).await?;
            let stored = client
                .set_field_value(&card, &group.id, &field_id, &value)
                .await?;
            render_item(&stored, format, full)?;
        }
        Action::Clear { card, group, field } => {
            let group = resolve_group(client, &card, &group).await?;
            let field_id = resolve_field(client, &group, &field).await?;
            client
                .clear_field_value(&card, &group.id, &field_id)
                .await?;
            render_message("Custom field value cleared.", format)?;
        }
    }
    Ok(())
}
