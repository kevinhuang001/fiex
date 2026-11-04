//! Durable Applications tree. Folder and application IDs are stable VFS
//! identities; physical paths exist only for imported assets.
mod migration;
mod removal;
use crate::core::files::Entry;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Mutex, OnceLock},
    time::Instant,
};

pub const FOLDER_PREFIX: &str = "applications://folder/";
pub const APP_PREFIX: &str = "applications://app/";
const SCOPE: &str = "tree";
const KEY: &str = "state";
const SCHEMA: u32 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ApplicationsState {
    pub schema: u32,
    pub revision: u64,
    #[serde(default)]
    pub migration_id: Option<String>,
    #[serde(default)]
    pub folders: BTreeMap<String, VirtualFolder>,
    #[serde(default)]
    pub apps: BTreeMap<String, ApplicationNode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VirtualFolder {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub order: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ApplicationNode {
    pub id: String,
    pub parent: Option<String>,
    pub display_name: String,
    pub source: ApplicationSource,
    pub order: i64,
    #[serde(default)]
    pub unavailable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ApplicationSource {
    System {
        stable_locator: String,
    },
    Imported {
        asset_id: String,
        #[serde(default)]
        extension: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingKind {
    Folder,
    Application,
}

#[derive(Clone, Debug)]
pub struct PendingTarget {
    pub uri: String,
    pub parent_uri: String,
    pub name: String,
    pub asset_id: Option<String>,
    pub extension: String,
    pub kind: PendingKind,
}

#[derive(Clone, Debug)]
struct PendingReservation {
    target: PendingTarget,
    created_at: Instant,
}

fn pending_reservations() -> &'static Mutex<HashMap<String, PendingReservation>> {
    static RESERVATIONS: OnceLock<Mutex<HashMap<String, PendingReservation>>> = OnceLock::new();
    RESERVATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn storage() -> crate::infrastructure::provider_storage::native::NativeScopedStorage {
    super::super::native_vfs::management_storage_for_provider("core:applications", SCOPE)
        .expect("Applications provider registration binds its storage owner")
}

pub fn snapshot() -> Result<ApplicationsState, String> {
    migration::ensure_migrated()?;
    removal::recover_pending()?;
    let Some(value) = storage().get(KEY)? else {
        return Ok(ApplicationsState {
            schema: SCHEMA,
            ..Default::default()
        });
    };
    let state: ApplicationsState = serde_json::from_value(value)
        .map_err(|error| format!("Applications state is corrupted: {error}"))?;
    if state.schema != SCHEMA {
        return Err("unsupported Applications state schema".into());
    }
    validate_state(&state)?;
    Ok(state)
}

fn update<F>(mut mutate: F) -> Result<ApplicationsState, String>
where
    F: FnMut(&mut ApplicationsState) -> Result<(), String>,
{
    migration::ensure_migrated()?;
    storage().update(KEY, |current: Option<ApplicationsState>| {
        let mut state = current.unwrap_or_else(|| ApplicationsState {
            schema: SCHEMA,
            ..Default::default()
        });
        if state.schema != SCHEMA {
            return Err("unsupported Applications state schema".into());
        }
        let previous = state.clone();
        mutate(&mut state)?;
        if state == previous {
            return Ok(state);
        }
        state.revision = state.revision.wrapping_add(1);
        validate_state(&state)?;
        Ok(state)
    })
}

fn validate_state(state: &ApplicationsState) -> Result<(), String> {
    for (id, folder) in &state.folders {
        if id != &folder.id || !valid_id(id) || folder.name.is_empty() {
            return Err("invalid Applications folder state".into());
        }
        if folder
            .parent
            .as_ref()
            .is_some_and(|parent| !state.folders.contains_key(parent))
        {
            return Err("Applications folder parent is missing".into());
        }
        let mut visited = std::collections::HashSet::new();
        let mut ancestor = folder.parent.as_deref();
        while let Some(parent) = ancestor {
            if !visited.insert(parent) || parent == id {
                return Err("Applications folder cycle detected".into());
            }
            ancestor = state
                .folders
                .get(parent)
                .and_then(|node| node.parent.as_deref());
        }
    }
    for (id, app) in &state.apps {
        if id != &app.id || !valid_id(id) || app.display_name.is_empty() {
            return Err("invalid Applications entry state".into());
        }
        if app
            .parent
            .as_ref()
            .is_some_and(|parent| !state.folders.contains_key(parent))
        {
            return Err("Applications entry parent is missing".into());
        }
        match &app.source {
            ApplicationSource::System { stable_locator } if stable_locator.is_empty() => {
                return Err("system application locator is empty".into());
            }
            ApplicationSource::Imported {
                asset_id,
                extension,
            } if !valid_id(asset_id) || !valid_extension(extension) => {
                return Err("invalid imported application asset id".into());
            }
            _ => {}
        }
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_extension(value: &str) -> bool {
    value.len() <= 16 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

pub fn folder_uri(id: &str) -> String {
    format!("{FOLDER_PREFIX}{id}")
}

pub fn app_uri(id: &str) -> String {
    format!("{APP_PREFIX}{id}")
}

pub fn folder_id(uri: &str) -> Option<String> {
    let id = uri.strip_prefix(FOLDER_PREFIX)?;
    (valid_id(id) && !id.contains('/')).then(|| id.to_owned())
}

pub fn app_id(uri: &str) -> Option<String> {
    let id = uri.strip_prefix(APP_PREFIX)?;
    (valid_id(id) && !id.contains('/')).then(|| id.to_owned())
}

pub fn is_folder(uri: &str) -> bool {
    uri == super::URI || folder_id(uri).is_some()
}

pub fn is_app(uri: &str) -> bool {
    app_id(uri).is_some()
}

pub fn is_user_app(uri: &str) -> bool {
    app_id(uri).is_some_and(|id| {
        snapshot()
            .ok()
            .and_then(|state| state.apps.get(&id).map(|app| app.source.clone()))
            .is_some_and(|source| matches!(source, ApplicationSource::Imported { .. }))
    })
}

pub fn reserve_child(parent_uri: &str, name: &str, kind: PendingKind) -> Result<String, String> {
    crate::core::files::validate_name(name)?;
    let parent = folder_id_or_root(parent_uri)?;
    let state = snapshot()?;
    require_parent(&state, parent.as_deref())?;
    if let Some(folder) = state
        .folders
        .values()
        .find(|folder| folder.parent == parent && folder.name.eq_ignore_ascii_case(name))
    {
        return Ok(folder_uri(&folder.id));
    }
    if let Some(app) = state
        .apps
        .values()
        .find(|app| app.parent == parent && app.display_name.eq_ignore_ascii_case(name))
    {
        return Ok(app_uri(&app.id));
    }
    ensure_unique_name(&state, parent.as_deref(), name, None)?;
    let mut reservations = pending_reservations()
        .lock()
        .map_err(|_| "Applications reservation registry unavailable")?;
    reservations.retain(|_, pending| pending.created_at.elapsed().as_secs() < 600);
    if let Some(existing) = reservations
        .values()
        .find(|pending| pending.target.parent_uri == parent_uri && pending.target.name == name)
    {
        if existing.target.kind == kind {
            return Ok(existing.target.uri.clone());
        }
        return Err("Conflict: application child already reserved with a different kind".into());
    }
    let id = random_id()?;
    let extension = std::path::Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !valid_extension(&extension) {
        return Err("invalid application file extension".into());
    }
    let target = PendingTarget {
        uri: match kind {
            PendingKind::Folder => folder_uri(&id),
            PendingKind::Application => app_uri(&id),
        },
        parent_uri: parent_uri.to_owned(),
        name: name.to_owned(),
        asset_id: (kind == PendingKind::Application)
            .then(random_id)
            .transpose()?,
        extension,
        kind,
    };
    reservations.insert(
        target.uri.clone(),
        PendingReservation {
            target: target.clone(),
            created_at: Instant::now(),
        },
    );
    Ok(target.uri)
}

pub fn reserved_child(parent_uri: &str, name: &str) -> Result<PendingTarget, String> {
    let reservations = pending_reservations()
        .lock()
        .map_err(|_| "Applications reservation registry unavailable")?;
    reservations
        .values()
        .find(|pending| {
            pending.target.parent_uri == parent_uri
                && pending.target.name == name
                && pending.created_at.elapsed().as_secs() < 600
        })
        .map(|pending| pending.target.clone())
        .ok_or_else(|| "NotFound: Applications child reservation".into())
}

pub fn pending_target(uri: &str) -> Option<PendingTarget> {
    pending_reservations()
        .lock()
        .ok()?
        .get(uri)
        .filter(|pending| pending.created_at.elapsed().as_secs() < 600)
        .map(|pending| pending.target.clone())
}

pub fn cancel_reservation(uri: &str) {
    if let Ok(mut reservations) = pending_reservations().lock() {
        reservations.remove(uri);
    }
}

pub fn commit_reserved_folder(uri: &str) -> Result<Entry, String> {
    let target = pending_target(uri).ok_or("NotFound: Applications child reservation")?;
    if target.kind != PendingKind::Folder {
        return Err("InvalidVfsUri: reserved target is not a folder".into());
    }
    let id = folder_id(uri).ok_or("invalid reserved folder URI")?;
    let parent = folder_id_or_root(&target.parent_uri)?;
    let state = update(|state| {
        require_parent(state, parent.as_deref())?;
        ensure_unique_name(state, parent.as_deref(), &target.name, None)?;
        if state.folders.contains_key(&id) {
            return Err("Conflict: application folder already exists".into());
        }
        let order = next_order(state, parent.as_deref());
        state.folders.insert(
            id.clone(),
            VirtualFolder {
                id: id.clone(),
                parent: parent.clone(),
                name: target.name.clone(),
                order,
            },
        );
        Ok(())
    })?;
    cancel_reservation(uri);
    Ok(folder_entry(
        state.folders.get(&id).expect("folder was inserted"),
    ))
}

pub fn commit_reserved_application(uri: &str) -> Result<Entry, String> {
    let target = pending_target(uri).ok_or("NotFound: Applications child reservation")?;
    if target.kind != PendingKind::Application {
        return Err("InvalidVfsUri: reserved target is not an application".into());
    }
    let id = app_id(uri).ok_or("invalid reserved application URI")?;
    let asset_id = target
        .asset_id
        .clone()
        .ok_or("missing application asset ID")?;
    let asset = imported_asset_path(&asset_id, &target.extension)?;
    if !asset.exists() {
        return Err("application asset was not committed".into());
    }
    let parent = folder_id_or_root(&target.parent_uri)?;
    let state = update(|state| {
        require_parent(state, parent.as_deref())?;
        ensure_unique_name(state, parent.as_deref(), &target.name, None)?;
        if state.apps.contains_key(&id) {
            return Err("Conflict: application already exists".into());
        }
        let order = next_order(state, parent.as_deref());
        state.apps.insert(
            id.clone(),
            ApplicationNode {
                id: id.clone(),
                parent: parent.clone(),
                display_name: target.name.clone(),
                source: ApplicationSource::Imported {
                    asset_id: asset_id.clone(),
                    extension: target.extension.clone(),
                },
                order,
                unavailable: false,
            },
        );
        Ok(())
    })?;
    cancel_reservation(uri);
    Ok(app_entry(
        state.apps.get(&id).expect("application was inserted"),
    ))
}

pub fn asset_path(uri: &str) -> Result<std::path::PathBuf, String> {
    if let Some(target) = pending_target(uri) {
        if target.kind != PendingKind::Application {
            return Err("folder has no application asset".into());
        }
        return imported_asset_path(
            target
                .asset_id
                .as_deref()
                .ok_or("missing application asset ID")?,
            &target.extension,
        );
    }
    let app = source(uri)?;
    match app.source {
        ApplicationSource::Imported {
            asset_id,
            extension,
        } => imported_asset_path(&asset_id, &extension),
        ApplicationSource::System { .. } => Err("system application has no private asset".into()),
    }
}

pub fn folder_entry(folder: &VirtualFolder) -> Entry {
    let uri = folder_uri(&folder.id);
    // Remove is a supported operation for virtual folders. Whether a folder
    // is empty is checked by the Provider's remove preflight at execution.
    let operations = [
        "stat", "list", "search", "write", "mkdir", "rename", "remove",
    ];
    application_entry(folder.name.clone(), uri, true, &operations)
}

pub fn app_entry(app: &ApplicationNode) -> Entry {
    let operations: &[&str] = match &app.source {
        ApplicationSource::System { .. } => &["stat"],
        ApplicationSource::Imported { .. } => &["stat", "read", "remove", "rename"],
    };
    let mut entry = application_entry(
        app.display_name.clone(),
        app_uri(&app.id),
        false,
        operations,
    );
    if let ApplicationSource::System { stable_locator } = &app.source {
        entry.link_target = Some(stable_locator.clone());
    }
    entry
}

fn application_entry(name: String, path: String, is_dir: bool, operations: &[&str]) -> Entry {
    let modified = 0;
    Entry {
        extension: std::path::Path::new(&name)
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase(),
        path,
        name,
        is_dir,
        is_symlink: false,
        size: 0,
        modified,
        created: modified,
        mime: "application/x-fiex-application".into(),
        link_target: None,
        allowed_ops: operations
            .iter()
            .map(|operation| (*operation).into())
            .collect(),
        tags: vec![],
        has_cache: false,
        pinned: false,
        uploaded: false,
        storage_projection: Default::default(),
        sync: None,
    }
}

pub fn create_folder(parent_uri: &str, name: &str) -> Result<Entry, String> {
    crate::core::files::validate_name(name)?;
    let parent = folder_id_or_root(parent_uri)?;
    let id = random_id()?;
    let state = update(|state| {
        require_parent(state, parent.as_deref())?;
        ensure_unique_name(state, parent.as_deref(), name, None)?;
        state.folders.insert(
            id.clone(),
            VirtualFolder {
                id: id.clone(),
                parent: parent.clone(),
                name: name.to_owned(),
                order: next_order(state, parent.as_deref()),
            },
        );
        Ok(())
    })?;
    let folder = state.folders.get(&id).expect("folder was inserted");
    Ok(folder_entry(folder))
}

pub fn rename_folder(uri: &str, name: &str) -> Result<Entry, String> {
    crate::core::files::validate_name(name)?;
    let id = folder_id(uri).ok_or("Applications root cannot be renamed")?;
    let state = update(|state| {
        let current = state
            .folders
            .get(&id)
            .cloned()
            .ok_or_else(|| "NotFound: application folder".to_string())?;
        ensure_unique_name(state, current.parent.as_deref(), name, Some(&id))?;
        state.folders.get_mut(&id).unwrap().name = name.to_owned();
        Ok(())
    })?;
    Ok(folder_entry(state.folders.get(&id).unwrap()))
}

pub fn rename_imported(uri: &str, name: &str) -> Result<Entry, String> {
    crate::core::files::validate_name(name)?;
    let id = app_id(uri).ok_or("not an Applications app node")?;
    let state = update(|state| {
        let current = state
            .apps
            .get(&id)
            .cloned()
            .ok_or_else(|| "NotFound: application".to_string())?;
        if !matches!(current.source, ApplicationSource::Imported { .. }) {
            return Err("System applications cannot be renamed in Fiex".into());
        }
        ensure_unique_name_excluding(state, current.parent.as_deref(), name, None, Some(&id))?;
        state.apps.get_mut(&id).unwrap().display_name = name.to_owned();
        Ok(())
    })?;
    Ok(app_entry(state.apps.get(&id).unwrap()))
}

pub fn rename_node(uri: &str, parent_uri: &str, name: &str) -> Result<Entry, String> {
    crate::core::files::validate_name(name)?;
    let target_parent = folder_id_or_root(parent_uri)?;
    if let Some(id) = folder_id(uri) {
        let state = update(|state| {
            let current = state
                .folders
                .get(&id)
                .cloned()
                .ok_or_else(|| "NotFound: application folder".to_string())?;
            require_parent(state, target_parent.as_deref())?;
            ensure_unique_name_excluding(state, target_parent.as_deref(), name, Some(&id), None)?;
            let mut ancestor = target_parent.as_deref();
            while let Some(parent) = ancestor {
                if parent == id {
                    return Err("Cannot move a folder into its descendant".into());
                }
                ancestor = state
                    .folders
                    .get(parent)
                    .and_then(|folder| folder.parent.as_deref());
            }
            let order = next_order(state, target_parent.as_deref());
            let folder = state.folders.get_mut(&id).unwrap();
            folder.parent = target_parent.clone();
            folder.name = name.to_owned();
            folder.order = order;
            let _ = current;
            Ok(())
        })?;
        return Ok(folder_entry(state.folders.get(&id).unwrap()));
    }
    let id = app_id(uri).ok_or("not an Applications node")?;
    let state = update(|state| {
        let current = state
            .apps
            .get(&id)
            .cloned()
            .ok_or_else(|| "NotFound: application".to_string())?;
        if !matches!(current.source, ApplicationSource::Imported { .. }) {
            return Err("System applications cannot be renamed in Fiex".into());
        }
        require_parent(state, target_parent.as_deref())?;
        ensure_unique_name_excluding(state, target_parent.as_deref(), name, None, Some(&id))?;
        let order = next_order(state, target_parent.as_deref());
        let app = state.apps.get_mut(&id).unwrap();
        app.parent = target_parent.clone();
        app.display_name = name.to_owned();
        app.order = order;
        Ok(())
    })?;
    Ok(app_entry(state.apps.get(&id).unwrap()))
}

pub fn remove_node(uri: &str) -> Result<(), String> {
    if folder_id(uri).is_some() {
        return remove_empty_folder(uri);
    }
    remove_imported(uri)
}

pub fn validate_remove(uri: &str) -> Result<(), String> {
    if let Some(id) = folder_id(uri) {
        let state = snapshot()?;
        if !state.folders.contains_key(&id) {
            return Err("NotFound: application folder".into());
        }
        if state
            .folders
            .values()
            .any(|folder| folder.parent.as_deref() == Some(id.as_str()))
            || state
                .apps
                .values()
                .any(|app| app.parent.as_deref() == Some(id.as_str()))
        {
            return Err("Directory not empty".into());
        }
        return Ok(());
    }
    let app = source(uri)?;
    if matches!(app.source, ApplicationSource::Imported { .. }) {
        Ok(())
    } else {
        Err("System applications cannot be deleted in Fiex".into())
    }
}

pub fn remove_empty_folder(uri: &str) -> Result<(), String> {
    let id = folder_id(uri).ok_or("Applications root cannot be removed")?;
    update(|state| {
        let folder = state
            .folders
            .get(&id)
            .ok_or_else(|| "NotFound: application folder".to_string())?;
        if state
            .folders
            .values()
            .any(|child| child.parent.as_deref() == Some(id.as_str()))
            || state
                .apps
                .values()
                .any(|app| app.parent.as_deref() == Some(id.as_str()))
        {
            return Err("Directory not empty".into());
        }
        let _ = folder;
        state.folders.remove(&id);
        Ok(())
    })?;
    Ok(())
}

pub fn parent_uri(uri: &str) -> Result<String, String> {
    if let Some(pending) = pending_target(uri) {
        return Ok(pending.parent_uri);
    }
    if let Some(id) = folder_id(uri) {
        let state = snapshot()?;
        let folder = state
            .folders
            .get(&id)
            .ok_or_else(|| "NotFound: application folder".to_string())?;
        return Ok(folder
            .parent
            .as_deref()
            .map(folder_uri)
            .unwrap_or_else(|| super::URI.into()));
    }
    if let Some(id) = app_id(uri) {
        let state = snapshot()?;
        let app = state
            .apps
            .get(&id)
            .ok_or_else(|| "NotFound: application".to_string())?;
        return Ok(app
            .parent
            .as_deref()
            .map(folder_uri)
            .unwrap_or_else(|| super::URI.into()));
    }
    Err("invalid Applications node URI".into())
}

pub fn validate_child(parent_uri: &str, child_uri: &str) -> Result<(), String> {
    if folder_id_or_root(parent_uri).is_err() {
        return Err("invalid Applications parent URI".into());
    }
    if folder_id(child_uri).is_some() || app_id(child_uri).is_some() {
        if let Some(pending) = pending_target(child_uri) {
            if pending.parent_uri == parent_uri {
                return Ok(());
            }
        }
        let state = snapshot()?;
        let expected_parent = parent_id(parent_uri)?;
        let actual_parent = if let Some(id) = folder_id(child_uri) {
            state
                .folders
                .get(&id)
                .and_then(|folder| folder.parent.as_deref())
        } else {
            state
                .apps
                .get(&app_id(child_uri).expect("validated app URI"))
                .and_then(|app| app.parent.as_deref())
        };
        if actual_parent == expected_parent.as_deref() {
            return Ok(());
        }
    }
    Err("InvalidVfsUri: application is not a child of this folder".into())
}

fn parent_id(uri: &str) -> Result<Option<String>, String> {
    folder_id_or_root(uri)
}

pub fn move_nodes(uris: &[String], target_uri: &str) -> Result<(), String> {
    let target = folder_id_or_root(target_uri)?;
    let mut ids = Vec::new();
    for uri in uris {
        if let Some(id) = folder_id(uri) {
            ids.push((id, true));
        } else if let Some(id) = app_id(uri) {
            ids.push((id, false));
        } else {
            return Err("invalid Applications node".into());
        }
    }
    ids.sort();
    ids.dedup();
    update(|state| {
        require_parent(state, target.as_deref())?;
        let moving_folders = ids
            .iter()
            .filter_map(|(id, is_folder)| is_folder.then_some(id.as_str()))
            .collect::<std::collections::HashSet<_>>();
        let moving_apps = ids
            .iter()
            .filter_map(|(id, is_folder)| (!is_folder).then_some(id.as_str()))
            .collect::<std::collections::HashSet<_>>();
        let mut names = std::collections::HashSet::new();
        for (id, folder) in &ids {
            if *folder {
                let node = state
                    .folders
                    .get(id)
                    .ok_or_else(|| "NotFound: application folder".to_string())?;
                if node.parent == target || target.as_deref() == Some(id) {
                    if node.parent == target {
                        continue;
                    }
                    return Err("Cannot move an Applications folder into itself".into());
                }
                let name = node.name.to_lowercase();
                if !names.insert(name) {
                    return Err("同一目录中不能有同名项目".into());
                }
                ensure_name_available_except(
                    state,
                    target.as_deref(),
                    &node.name,
                    &moving_folders,
                    &moving_apps,
                )?;
                let mut ancestor = target.as_deref();
                while let Some(parent) = ancestor {
                    if parent == id {
                        return Err("Cannot move a folder into its descendant".into());
                    }
                    ancestor = state
                        .folders
                        .get(parent)
                        .and_then(|entry| entry.parent.as_deref());
                }
            } else {
                let node = state
                    .apps
                    .get(id)
                    .ok_or_else(|| "NotFound: application".to_string())?;
                if node.parent == target {
                    continue;
                }
                let name = node.display_name.to_lowercase();
                if !names.insert(name) {
                    return Err("同一目录中不能有同名项目".into());
                }
                ensure_name_available_except(
                    state,
                    target.as_deref(),
                    &node.display_name,
                    &moving_folders,
                    &moving_apps,
                )?;
            }
        }
        for (id, folder) in &ids {
            if *folder {
                if state
                    .folders
                    .get(id)
                    .is_some_and(|node| node.parent == target)
                {
                    continue;
                }
                let order = next_order(state, target.as_deref());
                let node = state.folders.get_mut(id).unwrap();
                node.parent = target.clone();
                node.order = order;
            } else {
                if state.apps.get(id).is_some_and(|node| node.parent == target) {
                    continue;
                }
                let order = next_order(state, target.as_deref());
                let node = state.apps.get_mut(id).unwrap();
                node.parent = target.clone();
                node.order = order;
            }
        }
        Ok(())
    })?;
    Ok(())
}

pub fn register_system_apps(
    records: &[(String, String)],
) -> Result<BTreeMap<String, String>, String> {
    let ids = records
        .iter()
        .map(|(locator, _)| (system_id(locator), locator.clone()))
        .collect::<Vec<_>>();
    update(|state| {
        for app in state.apps.values_mut() {
            if matches!(app.source, ApplicationSource::System { .. }) {
                app.unavailable = true;
            }
        }
        for ((_, name), (id, locator)) in records.iter().zip(ids.iter()) {
            match state.apps.get_mut(id) {
                Some(app) => {
                    app.display_name = name.clone();
                    app.unavailable = false;
                }
                None => {
                    state.apps.insert(
                        id.clone(),
                        ApplicationNode {
                            id: id.clone(),
                            parent: None,
                            display_name: name.clone(),
                            source: ApplicationSource::System {
                                stable_locator: locator.clone(),
                            },
                            order: next_order(state, None),
                            unavailable: false,
                        },
                    );
                }
            }
        }
        Ok(())
    })?;
    Ok(ids
        .into_iter()
        .map(|(id, locator)| (locator, app_uri(&id)))
        .collect::<BTreeMap<_, _>>())
}

pub fn imported_asset_path(asset_id: &str, extension: &str) -> Result<std::path::PathBuf, String> {
    if !valid_id(asset_id) || !valid_extension(extension) {
        return Err("invalid application asset ID".into());
    }
    let root = applications_root_path()?;
    let name = if extension.is_empty() {
        asset_id.to_owned()
    } else {
        format!("{asset_id}.{extension}")
    };
    Ok(root.join(name))
}

pub(super) fn applications_root_path() -> Result<std::path::PathBuf, String> {
    let fiex = dirs::home_dir()
        .ok_or("cannot locate home directory")?
        .join(".fiex");
    std::fs::create_dir_all(&fiex).map_err(|error| error.to_string())?;
    let fiex = dunce::canonicalize(fiex).map_err(|error| error.to_string())?;
    let root = fiex.join("applications");
    match std::fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("Applications asset root must be a private directory".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&root).map_err(|error| error.to_string())?;
        }
        Err(error) => return Err(error.to_string()),
    }
    let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
    if root.parent() != Some(fiex.as_path()) {
        return Err("Applications asset root escaped its private directory".into());
    }
    Ok(root)
}

pub fn folder_children(uri: &str) -> Result<Vec<Entry>, String> {
    let parent = folder_id_or_root(uri)?;
    let state = snapshot()?;
    if let Some(id) = parent.as_deref() {
        require_parent(&state, Some(id))?;
    }
    let mut entries = state
        .folders
        .values()
        .filter(|folder| folder.parent == parent)
        .map(folder_entry)
        .chain(
            state
                .apps
                .values()
                .filter(|app| app.parent == parent)
                .map(app_entry),
        )
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.name.to_lowercase());
    Ok(entries)
}

pub fn entry(uri: &str) -> Result<Entry, String> {
    if uri == super::URI {
        let mut item = application_entry(
            "Applications".into(),
            uri.into(),
            true,
            &["stat", "list", "search", "write", "mkdir"],
        );
        item.mime = "inode/directory".into();
        return Ok(item);
    }
    let state = snapshot()?;
    if let Some(id) = folder_id(uri) {
        return state
            .folders
            .get(&id)
            .map(folder_entry)
            .ok_or_else(|| "NotFound: application folder".into());
    }
    if let Some(id) = app_id(uri) {
        return state
            .apps
            .get(&id)
            .map(app_entry)
            .ok_or_else(|| "NotFound: application".into());
    }
    Err("invalid Applications URI".into())
}

pub fn source(uri: &str) -> Result<ApplicationNode, String> {
    let id = app_id(uri).ok_or("not an Applications app node")?;
    snapshot()?
        .apps
        .get(&id)
        .cloned()
        .ok_or_else(|| "NotFound: application".into())
}

pub fn remove_imported(uri: &str) -> Result<(), String> {
    removal::remove(uri)
}

fn system_id(locator: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("sys-{}", hex::encode(Sha256::digest(locator.as_bytes())))
}

fn random_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|error| error.to_string())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let encoded = hex::encode(bytes);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &encoded[0..8],
        &encoded[8..12],
        &encoded[12..16],
        &encoded[16..20],
        &encoded[20..32]
    ))
}

fn folder_id_or_root(uri: &str) -> Result<Option<String>, String> {
    if uri == super::URI {
        Ok(None)
    } else {
        folder_id(uri)
            .map(Some)
            .ok_or_else(|| "invalid Applications folder URI".into())
    }
}

fn require_parent(state: &ApplicationsState, parent: Option<&str>) -> Result<(), String> {
    if parent.is_some_and(|id| !state.folders.contains_key(id)) {
        return Err("NotFound: application parent folder".into());
    }
    Ok(())
}

fn next_order(state: &ApplicationsState, parent: Option<&str>) -> i64 {
    state
        .folders
        .values()
        .filter(|folder| folder.parent.as_deref() == parent)
        .map(|folder| folder.order)
        .chain(
            state
                .apps
                .values()
                .filter(|app| app.parent.as_deref() == parent)
                .map(|app| app.order),
        )
        .max()
        .unwrap_or(-1)
        .saturating_add(1)
}

fn ensure_unique_name(
    state: &ApplicationsState,
    parent: Option<&str>,
    name: &str,
    except_folder: Option<&str>,
) -> Result<(), String> {
    ensure_unique_name_excluding(state, parent, name, except_folder, None)
}

fn ensure_unique_name_excluding(
    state: &ApplicationsState,
    parent: Option<&str>,
    name: &str,
    except_folder: Option<&str>,
    except_app: Option<&str>,
) -> Result<(), String> {
    let same = |candidate: &str| candidate.eq_ignore_ascii_case(name);
    if state.folders.values().any(|folder| {
        folder.parent.as_deref() == parent
            && Some(folder.id.as_str()) != except_folder
            && same(&folder.name)
    }) || state.apps.values().any(|app| {
        app.parent.as_deref() == parent
            && Some(app.id.as_str()) != except_app
            && same(&app.display_name)
    }) {
        return Err(crate::i18n::t("同名项目已存在，请更换名称").into());
    }
    Ok(())
}

fn ensure_name_available_except(
    state: &ApplicationsState,
    parent: Option<&str>,
    name: &str,
    moving_folders: &std::collections::HashSet<&str>,
    moving_apps: &std::collections::HashSet<&str>,
) -> Result<(), String> {
    if state.folders.values().any(|folder| {
        folder.parent.as_deref() == parent
            && !moving_folders.contains(folder.id.as_str())
            && folder.name.eq_ignore_ascii_case(name)
    }) || state.apps.values().any(|app| {
        app.parent.as_deref() == parent
            && !moving_apps.contains(app.id.as_str())
            && app.display_name.eq_ignore_ascii_case(name)
    }) {
        return Err(crate::i18n::t("同名项目已存在，请更换名称").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: &str, parent: Option<&str>) -> VirtualFolder {
        VirtualFolder {
            id: id.into(),
            parent: parent.map(str::to_owned),
            name: id.into(),
            order: 0,
        }
    }

    #[test]
    fn virtual_identity_is_opaque_and_independent_of_display_name() {
        let id = "folder-0123456789abcdef";
        let before = folder_uri(id);
        let mut node = folder(id, None);
        node.name = "Design".into();
        let after_rename = folder_uri(&node.id);
        assert_eq!(before, after_rename);
        assert_eq!(folder_id(&before).as_deref(), Some(id));
        assert!(folder_id("applications://folder/../escape").is_none());
    }

    #[test]
    fn state_validation_rejects_cycles_and_missing_parents() {
        let mut state = ApplicationsState {
            schema: SCHEMA,
            ..Default::default()
        };
        state
            .folders
            .insert("folder-a".into(), folder("folder-a", Some("folder-b")));
        state
            .folders
            .insert("folder-b".into(), folder("folder-b", Some("folder-a")));
        assert!(validate_state(&state).unwrap_err().contains("cycle"));

        let mut state = ApplicationsState {
            schema: SCHEMA,
            ..Default::default()
        };
        state
            .folders
            .insert("folder-a".into(), folder("folder-a", Some("missing")));
        assert!(validate_state(&state).unwrap_err().contains("parent"));
    }

    #[test]
    fn system_application_identity_uses_stable_locator_only() {
        assert_eq!(
            system_id("/Applications/Editor.app"),
            system_id("/Applications/Editor.app")
        );
        assert_ne!(
            system_id("/Applications/Editor.app"),
            system_id("/Applications/Other.app")
        );
    }
}
