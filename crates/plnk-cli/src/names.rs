//! Name resolution and output decoration for the `--names` / `names = true`
//! mode.
//!
//! [`NameIndex`] does two jobs:
//!
//! * **Input** — resolve a name given to a foreign-key argument to the ID the
//!   API needs. Matching is ID-first, then the house three-tier name match.
//! * **Output** — remember ID→name pairs and decorate rendered values with
//!   them: sibling `*Name` fields in JSON, substituted values in table and
//!   markdown output.
//!
//! The mechanism is mapping-driven. `kind_for_field` maps a serde JSON field
//! (for example `boardId`) to a [`NameKind`], and `name_field` derives the
//! sibling name field. Each resource adds its mapping arm and its resolver; the
//! decoration loop itself is shared.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::{Map, Value};

use plnk_core::api::{
    BoardApi, CustomFieldApi, CustomFieldGroupApi, LabelApi, ListApi, Named, ProjectApi, UserApi,
    match_by_name,
};
use plnk_core::error::PlankaError;
use plnk_core::models::{Board, Label, List, Project, User};

/// The kind of resource a foreign-key field points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind {
    #[allow(dead_code)] // wired up by the resource commits
    Project,
    #[allow(dead_code)]
    Board,
    #[allow(dead_code)]
    List,
    #[allow(dead_code)]
    User,
    #[allow(dead_code)]
    Label,
    #[allow(dead_code)]
    CustomFieldGroup,
    #[allow(dead_code)]
    BaseCustomFieldGroup,
    #[allow(dead_code)]
    CustomField,
}

/// The bundle of API capabilities the name resolvers need.
///
/// A blanket impl covers any client that implements every resource trait, so
/// commands can keep their existing narrow bounds and just add `NameSource`.
pub trait NameSource:
    ProjectApi + BoardApi + ListApi + UserApi + LabelApi + CustomFieldGroupApi + CustomFieldApi
{
}

impl<T> NameSource for T where
    T: ProjectApi + BoardApi + ListApi + UserApi + LabelApi + CustomFieldGroupApi + CustomFieldApi
{
}

/// ID→name cache plus the enum/search caches the resolvers need.
#[derive(Default)]
pub struct NameIndex {
    enabled: bool,
    names: HashMap<(NameKind, String), String>,
    missing: HashSet<(NameKind, String)>,
    /// Board ID → project ID, learned while resolving boards.
    board_project: HashMap<String, String>,
    /// List ID → board ID, learned while resolving lists.
    list_board: HashMap<String, String>,
    /// Enumeration cache for project name search.
    projects: Option<Vec<Project>>,
    /// Enumeration cache for board name search.
    boards: Option<Vec<Board>>,
    /// Enumeration cache for list name search.
    lists: Option<Vec<List>>,
    /// Enumeration cache for user name search.
    users: Option<Vec<User>>,
    /// Enumeration cache for label name search.
    labels: Option<Vec<Label>>,
    /// Every custom field group, ordinary and base, with resolved display
    /// names (adopted groups take their name from the base group).
    groups: Option<Vec<GroupEntry>>,
    /// The base-group subset of [`groups`](Self::groups).
    base_groups: Option<Vec<GroupEntry>>,
    /// The ordinary (board/card) group subset of [`groups`](Self::groups).
    ordinary_groups: Option<Vec<GroupEntry>>,
    /// Whether every group's fields have been loaded into the cache.
    fields_loaded: bool,
}

impl NameIndex {
    /// Create an index. A disabled index performs no lookups and decorates
    /// nothing.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    /// Remember a known ID→name pair.
    fn remember(&mut self, kind: NameKind, id: &str, name: &str) {
        self.names.insert((kind, id.to_string()), name.to_string());
    }

    /// Look up a cached name.
    fn lookup(&self, kind: NameKind, id: &str) -> Option<&str> {
        self.names.get(&(kind, id.to_string())).map(String::as_str)
    }

    /// The JSON field added for a resolved name.
    fn name_field(kind: NameKind) -> &'static str {
        match kind {
            NameKind::Project => "projectName",
            NameKind::Board => "boardName",
            NameKind::List => "listName",
            NameKind::User => "userName",
            NameKind::Label => "labelName",
            NameKind::CustomFieldGroup => "customFieldGroupName",
            NameKind::BaseCustomFieldGroup => "baseCustomFieldGroupName",
            NameKind::CustomField => "customFieldName",
        }
    }

    /// Map a foreign-key JSON field to the resource it points at.
    ///
    /// Each resource commit adds its own arm here.
    fn kind_for_field(field: &str) -> Option<NameKind> {
        match field {
            "projectId" => Some(NameKind::Project),
            "boardId" => Some(NameKind::Board),
            "listId" => Some(NameKind::List),
            "userId" => Some(NameKind::User),
            "labelId" => Some(NameKind::Label),
            "customFieldGroupId" => Some(NameKind::CustomFieldGroup),
            "baseCustomFieldGroupId" => Some(NameKind::BaseCustomFieldGroup),
            "customFieldId" => Some(NameKind::CustomField),
            _ => None,
        }
    }

    /// Substitute a foreign-key value for display in table/markdown output.
    #[must_use]
    pub(crate) fn display(&self, field: &str, id: &str) -> Option<&str> {
        if !self.enabled {
            return None;
        }
        Self::kind_for_field(field).and_then(|kind| self.lookup(kind, id))
    }

    /// Resolve one ID and cache the result. A resolver returning `None` marks
    /// the ID as unresolvable so it is not retried.
    async fn ensure<C>(&mut self, client: &C, kind: NameKind, id: &str) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled
            || self.names.contains_key(&(kind, id.to_string()))
            || self.missing.contains(&(kind, id.to_string()))
        {
            return Ok(());
        }

        match self.resolve_kind(client, kind, id).await? {
            Some(name) => self.remember(kind, id, &name),
            None => {
                self.missing.insert((kind, id.to_string()));
            }
        }
        Ok(())
    }

    /// Resolve a single ID to a name. Each resource commit adds its arm.
    async fn resolve_kind<C>(
        &mut self,
        client: &C,
        kind: NameKind,
        id: &str,
    ) -> Result<Option<String>, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        match kind {
            NameKind::Project => {
                let project = client.get_project(id).await?;
                Ok(Some(project.name))
            }
            NameKind::Board => {
                let board = client.get_board(id).await?;
                self.board_project
                    .insert(board.id.clone(), board.project_id.clone());
                Ok(Some(board.name))
            }
            NameKind::List => {
                let list = client.get_list(id).await?;
                self.list_board
                    .insert(list.id.clone(), list.board_id.clone());
                Ok(Some(list.name))
            }
            NameKind::User => {
                let user = client.get_user(id).await?;
                Ok(Some(user.username.unwrap_or(user.name)))
            }
            NameKind::Label => {
                // There is no label-by-id endpoint; labels are only reachable
                // through their board. Enumerate, then read from the cache.
                self.ensure_labels(client).await?;
                Ok(self.lookup(NameKind::Label, id).map(str::to_string))
            }
            NameKind::CustomFieldGroup => {
                let group = client.get_field_group(id).await?;
                if let Some(name) = group.name {
                    return Ok(Some(name));
                }
                // An adopted group's name lives on the base group.
                if let Some(base_id) = group.base_custom_field_group_id {
                    let base = client.get_base_field_group(&base_id).await?;
                    return Ok(Some(base.name));
                }
                Ok(None)
            }
            NameKind::BaseCustomFieldGroup => {
                let base = client.get_base_field_group(id).await?;
                Ok(Some(base.name))
            }
            NameKind::CustomField => {
                // There is no field-by-id endpoint; fields belong to a group.
                // Enumerate every group's fields, then read from the cache.
                self.ensure_fields(client).await?;
                Ok(self.lookup(NameKind::CustomField, id).map(str::to_string))
            }
        }
    }

    /// Load and cache every project, remembering each ID→name pair so a later
    /// decoration does not re-fetch what the search already saw.
    async fn ensure_projects<C>(&mut self, client: &C) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if self.projects.is_some() {
            return Ok(());
        }
        let projects = client.list_projects().await?;
        for project in &projects {
            self.remember(NameKind::Project, &project.id, &project.name);
        }
        self.projects = Some(projects);
        Ok(())
    }

    /// Resolve a project name (or ID) to an ID.
    pub async fn resolve_project<C>(
        &mut self,
        client: &C,
        query: &str,
    ) -> Result<String, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(query.to_string());
        }
        self.ensure_projects(client).await?;
        let projects = self.projects.as_ref().expect("projects loaded");
        resolve_resource(projects, "--project", "project", query, |project| {
            project.id.clone()
        })
    }

    /// Load and cache every board across every project.
    async fn ensure_boards<C>(&mut self, client: &C) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if self.boards.is_some() {
            return Ok(());
        }
        self.ensure_projects(client).await?;
        let project_ids: Vec<String> = self
            .projects
            .as_ref()
            .expect("projects loaded")
            .iter()
            .map(|project| project.id.clone())
            .collect();

        let mut boards = Vec::new();
        for project_id in project_ids {
            for board in client.list_boards(&project_id).await? {
                self.remember(NameKind::Board, &board.id, &board.name);
                self.board_project
                    .insert(board.id.clone(), board.project_id.clone());
                boards.push(board);
            }
        }
        self.boards = Some(boards);
        Ok(())
    }

    /// Resolve a board name (or ID) to an ID.
    ///
    /// When a project ID is supplied, only that project's boards are
    /// considered.
    pub async fn resolve_board<C>(
        &mut self,
        client: &C,
        query: &str,
        project: Option<&str>,
    ) -> Result<String, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(query.to_string());
        }
        self.ensure_boards(client).await?;
        let boards = self.boards.as_ref().expect("boards loaded");
        let candidates: Vec<Board> = match project {
            Some(project_id) => boards
                .iter()
                .filter(|board| board.project_id == project_id)
                .cloned()
                .collect(),
            None => boards.clone(),
        };
        resolve_resource(&candidates, "--board", "board", query, |board| {
            board.id.clone()
        })
    }

    /// Load and cache every list across every board.
    async fn ensure_lists<C>(&mut self, client: &C) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if self.lists.is_some() {
            return Ok(());
        }
        self.ensure_boards(client).await?;
        let board_ids: Vec<String> = self
            .boards
            .as_ref()
            .expect("boards loaded")
            .iter()
            .map(|board| board.id.clone())
            .collect();

        let mut lists = Vec::new();
        for board_id in board_ids {
            for list in client.list_lists(&board_id).await? {
                self.remember(NameKind::List, &list.id, &list.name);
                self.list_board
                    .insert(list.id.clone(), list.board_id.clone());
                lists.push(list);
            }
        }
        self.lists = Some(lists);
        Ok(())
    }

    /// Resolve a list name (or ID) to an ID.
    ///
    /// The search is scoped to a board when one is given, then to the boards of
    /// a project, otherwise it spans every board.
    pub async fn resolve_list<C>(
        &mut self,
        client: &C,
        query: &str,
        board: Option<&str>,
        project: Option<&str>,
    ) -> Result<String, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(query.to_string());
        }
        self.ensure_lists(client).await?;
        let lists = self.lists.as_ref().expect("lists loaded");
        let candidates: Vec<List> = if let Some(board_id) = board {
            lists
                .iter()
                .filter(|list| list.board_id == board_id)
                .cloned()
                .collect()
        } else if let Some(project_id) = project {
            let board_ids: HashSet<&str> = self
                .boards
                .as_ref()
                .expect("boards loaded")
                .iter()
                .filter(|board| board.project_id == project_id)
                .map(|board| board.id.as_str())
                .collect();
            lists
                .iter()
                .filter(|list| board_ids.contains(list.board_id.as_str()))
                .cloned()
                .collect()
        } else {
            lists.clone()
        };
        resolve_resource(&candidates, "--list", "list", query, |list| list.id.clone())
    }

    /// Load and cache every user, keyed on the unique username.
    async fn ensure_users<C>(&mut self, client: &C) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if self.users.is_some() {
            return Ok(());
        }
        let users = client.list_users().await?;
        for user in &users {
            self.remember(NameKind::User, &user.id, username_of(user));
        }
        self.users = Some(users);
        Ok(())
    }

    /// Load and cache every label across every board. Unnamed labels are not
    /// remembered: there is no name to show for them.
    async fn ensure_labels<C>(&mut self, client: &C) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if self.labels.is_some() {
            return Ok(());
        }
        self.ensure_boards(client).await?;
        let board_ids: Vec<String> = self
            .boards
            .as_ref()
            .expect("boards loaded")
            .iter()
            .map(|board| board.id.clone())
            .collect();

        let mut labels = Vec::new();
        for board_id in board_ids {
            for label in client.list_labels(&board_id).await? {
                if let Some(name) = &label.name {
                    self.remember(NameKind::Label, &label.id, name);
                }
                labels.push(label);
            }
        }
        self.labels = Some(labels);
        Ok(())
    }

    /// Load and cache every custom field group, ordinary and base.
    async fn ensure_groups<C>(&mut self, client: &C) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if self.groups.is_some() {
            return Ok(());
        }

        let base_groups = client.list_all_base_field_groups().await?;
        let mut base_names: HashMap<String, String> = HashMap::new();
        let mut base_entries = Vec::new();
        for base in &base_groups {
            self.remember(NameKind::BaseCustomFieldGroup, &base.id, &base.name);
            base_names.insert(base.id.clone(), base.name.clone());
            base_entries.push(GroupEntry {
                id: base.id.clone(),
                name: base.name.clone(),
            });
        }

        self.ensure_boards(client).await?;
        let board_ids: Vec<String> = self
            .boards
            .as_ref()
            .expect("boards loaded")
            .iter()
            .map(|board| board.id.clone())
            .collect();

        let mut ordinary_entries = Vec::new();
        for board_id in board_ids {
            for group in client.list_field_groups_for_board(&board_id).await? {
                let display = group.name.clone().or_else(|| {
                    group
                        .base_custom_field_group_id
                        .as_ref()
                        .and_then(|base_id| base_names.get(base_id).cloned())
                });
                if let Some(name) = display {
                    self.remember(NameKind::CustomFieldGroup, &group.id, &name);
                    ordinary_entries.push(GroupEntry {
                        id: group.id.clone(),
                        name,
                    });
                }
            }
        }

        let mut entries = base_entries.clone();
        entries.extend(ordinary_entries.iter().cloned());
        self.base_groups = Some(base_entries);
        self.ordinary_groups = Some(ordinary_entries);
        self.groups = Some(entries);
        Ok(())
    }

    /// Load and cache every custom field across every group.
    async fn ensure_fields<C>(&mut self, client: &C) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if self.fields_loaded {
            return Ok(());
        }
        self.ensure_groups(client).await?;
        let base_groups = self.base_groups.clone().unwrap_or_default();
        let ordinary_groups = self.ordinary_groups.clone().unwrap_or_default();

        for group in &base_groups {
            for field in client.list_fields(&group.id, true).await? {
                self.remember(NameKind::CustomField, &field.id, &field.name);
            }
        }
        for group in &ordinary_groups {
            for field in client.list_fields(&group.id, false).await? {
                self.remember(NameKind::CustomField, &field.id, &field.name);
            }
        }
        self.fields_loaded = true;
        Ok(())
    }

    /// Resolve a custom field group name (or ID) to an ID.
    pub async fn resolve_group<C>(&mut self, client: &C, query: &str) -> Result<String, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(query.to_string());
        }
        self.ensure_groups(client).await?;
        let groups = self.groups.as_ref().expect("groups loaded");
        resolve_resource(groups, "--group", "custom field group", query, |group| {
            group.id.clone()
        })
    }

    /// Resolve a base custom field group name (or ID) to an ID.
    pub async fn resolve_base_group<C>(
        &mut self,
        client: &C,
        query: &str,
    ) -> Result<String, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(query.to_string());
        }
        self.ensure_groups(client).await?;
        let groups = self.base_groups.as_ref().expect("base groups loaded");
        resolve_resource(
            groups,
            "--base",
            "base custom field group",
            query,
            |group| group.id.clone(),
        )
    }

    /// Resolve a label name (or ID) to an ID, optionally scoped to a board.
    pub async fn resolve_label<C>(
        &mut self,
        client: &C,
        query: &str,
        board: Option<&str>,
    ) -> Result<String, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(query.to_string());
        }
        self.ensure_labels(client).await?;
        let labels = self.labels.as_ref().expect("labels loaded");
        let candidates: Vec<Label> = match board {
            Some(board_id) => labels
                .iter()
                .filter(|label| label.board_id == board_id)
                .cloned()
                .collect(),
            None => labels.clone(),
        };
        resolve_resource(&candidates, "--label", "label", query, |label| {
            label.id.clone()
        })
    }

    /// Resolve a username (or user ID) to an ID.
    pub async fn resolve_user<C>(&mut self, client: &C, query: &str) -> Result<String, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(query.to_string());
        }
        self.ensure_users(client).await?;
        let users = self.users.as_ref().expect("users loaded");
        let candidates: Vec<Username> = users.iter().map(Username).collect();
        resolve_resource(&candidates, "--user", "user", query, |user| {
            user.0.id.clone()
        })
    }

    /// Resolve an optional `--list` / `--to-list` value.
    pub async fn resolve_optional_list<C>(
        &mut self,
        client: &C,
        list: Option<String>,
        board: Option<&str>,
        project: Option<&str>,
    ) -> Result<Option<String>, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        match list {
            Some(list) => Ok(Some(
                self.resolve_list(client, &list, board, project).await?,
            )),
            None => Ok(None),
        }
    }

    /// Resolve an optional `--board` / `--to-board` value.
    pub async fn resolve_optional_board<C>(
        &mut self,
        client: &C,
        board: Option<String>,
        project: Option<&str>,
    ) -> Result<Option<String>, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        match board {
            Some(board) => Ok(Some(self.resolve_board(client, &board, project).await?)),
            None => Ok(None),
        }
    }

    /// Resolve an optional `--project` value.
    pub async fn resolve_optional_project<C>(
        &mut self,
        client: &C,
        project: Option<String>,
    ) -> Result<Option<String>, PlankaError>
    where
        C: NameSource + ?Sized,
    {
        match project {
            Some(project) => Ok(Some(self.resolve_project(client, &project).await?)),
            None => Ok(None),
        }
    }

    /// Resolve every foreign key reachable in a JSON value.
    pub async fn prefetch_value<C>(&mut self, client: &C, value: &Value) -> Result<(), PlankaError>
    where
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(());
        }

        let mut needed = Vec::new();
        collect_foreign_keys(value, &mut needed);
        for (kind, id) in needed {
            self.ensure(client, kind, &id).await?;
        }
        Ok(())
    }

    /// Resolve every foreign key reachable in a serializable item.
    pub async fn prefetch_item<T, C>(&mut self, client: &C, item: &T) -> Result<(), PlankaError>
    where
        T: Serialize,
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(());
        }
        let value = serde_json::to_value(item)?;
        self.prefetch_value(client, &value).await
    }

    /// Resolve every foreign key reachable in a collection of items.
    pub async fn prefetch_items<T, C>(&mut self, client: &C, items: &[T]) -> Result<(), PlankaError>
    where
        T: Serialize,
        C: NameSource + ?Sized,
    {
        if !self.enabled {
            return Ok(());
        }
        let value = serde_json::to_value(items)?;
        self.prefetch_value(client, &value).await
    }

    /// Insert sibling `*Name` fields into a JSON value in place.
    pub fn decorate_json(&self, value: &mut Value) {
        if !self.enabled {
            return;
        }
        match value {
            Value::Object(object) => {
                let insertions = self.name_insertions(object);
                for (field, name) in insertions {
                    object.insert(field, Value::String(name));
                }
                for child in object.values_mut() {
                    self.decorate_json(child);
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.decorate_json(item);
                }
            }
            _ => {}
        }
    }

    /// Compute the name fields to add to one JSON object.
    fn name_insertions(&self, object: &Map<String, Value>) -> Vec<(String, String)> {
        let mut insertions = Vec::new();

        for (field, value) in object {
            let Some(kind) = Self::kind_for_field(field) else {
                continue;
            };
            let Some(id) = value.as_str() else {
                continue;
            };
            if let Some(name) = self.lookup(kind, id) {
                let name_field = Self::name_field(kind);
                if !object.contains_key(name_field) {
                    insertions.push((name_field.to_string(), name.to_string()));
                }
            }
        }

        // A list or card carries no `projectId`; derive it from its parent
        // chain so the item still reports the project it belongs to. Only do
        // this once a project-reaching mapping has been registered, so the
        // harness stays inert until a resource commit enables it.
        if !object.contains_key("projectName") && Self::project_mapping_active() {
            if let Some(project_id) = self.project_id_for(object) {
                if let Some(name) = self.lookup(NameKind::Project, &project_id) {
                    insertions.push(("projectName".to_string(), name.to_string()));
                }
            }
        }

        insertions
    }

    /// Whether any registered mapping can resolve a project name.
    fn project_mapping_active() -> bool {
        Self::kind_for_field("projectId").is_some()
            || Self::kind_for_field("boardId").is_some()
            || Self::kind_for_field("listId").is_some()
    }

    /// Walk `projectId` → `boardId` → `listId` to find the owning project.
    fn project_id_for(&self, object: &Map<String, Value>) -> Option<String> {
        if let Some(id) = object.get("projectId").and_then(Value::as_str) {
            return Some(id.to_string());
        }
        if let Some(board_id) = object.get("boardId").and_then(Value::as_str) {
            return self.board_project.get(board_id).cloned();
        }
        if let Some(list_id) = object.get("listId").and_then(Value::as_str) {
            let board_id = self.list_board.get(list_id)?;
            return self.board_project.get(board_id).cloned();
        }
        None
    }
}

/// A custom field group paired with the display name it presents.
#[derive(Clone)]
struct GroupEntry {
    id: String,
    name: String,
}

impl Named for GroupEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

/// The value a user is known by: the unique username, falling back to the
/// display name when Planka has no username.
fn username_of(user: &User) -> &str {
    user.username.as_deref().unwrap_or(&user.name)
}

/// Adapter so the three-tier matcher keys users on their username.
struct Username<'a>(&'a User);

impl Named for Username<'_> {
    fn name(&self) -> &str {
        username_of(self.0)
    }
}

/// Resolve an ID-or-name argument against a candidate set.
///
/// ID-first, then the house three-tier name match. Multiple matches are an
/// error that lists the candidates.
fn resolve_resource<T: Named>(
    items: &[T],
    flag: &str,
    label: &str,
    query: &str,
    id_of: impl Fn(&T) -> String,
) -> Result<String, PlankaError> {
    if let Some(found) = items.iter().find(|item| id_of(item) == query) {
        return Ok(id_of(found));
    }

    let matched = match_by_name(items, query);
    match matched.len() {
        0 => Err(PlankaError::NotFoundMessage {
            message: format!(
                "No {label} matching '{query}' was found. Use '{label} list' to \
                 inspect available {label}s or pass an ID."
            ),
        }),
        1 => Ok(id_of(matched[0])),
        _ => {
            let candidates = matched
                .iter()
                .map(|item| format!("{} ({})", item.name(), id_of(item)))
                .collect::<Vec<_>>()
                .join(", ");
            Err(PlankaError::InvalidOptionValue {
                field: flag.to_string(),
                message: format!(
                    "{label} '{query}' matched multiple entries. Be more specific or \
                     pass an ID. Matches: {candidates}"
                ),
            })
        }
    }
}

/// Recursively collect `(kind, id)` pairs from a JSON value.
fn collect_foreign_keys(value: &Value, out: &mut Vec<(NameKind, String)>) {
    match value {
        Value::Object(object) => {
            for (field, child) in object {
                if let Some(kind) = NameIndex::kind_for_field(field) {
                    if let Some(id) = child.as_str() {
                        out.push((kind, id.to_string()));
                    }
                }
                collect_foreign_keys(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_foreign_keys(item, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_index_decorates_nothing() {
        let index = NameIndex::new(false);
        let mut value = serde_json::json!({ "projectId": "p1" });
        index.decorate_json(&mut value);
        assert!(value.get("projectName").is_none());
    }

    #[test]
    fn enabled_index_inserts_mapped_name_fields() {
        let mut index = NameIndex::new(true);
        index.remember(NameKind::Project, "p1", "Platform");
        let mut value = serde_json::json!({ "projectId": "p1" });
        index.decorate_json(&mut value);
        assert_eq!(value["projectName"], "Platform");
    }

    #[test]
    fn enabled_index_omits_names_it_does_not_know() {
        let index = NameIndex::new(true);
        let mut value = serde_json::json!({ "projectId": "p1" });
        index.decorate_json(&mut value);
        assert!(value.get("projectName").is_none());
    }
}
