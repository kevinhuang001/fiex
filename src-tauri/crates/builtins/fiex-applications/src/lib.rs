//! Applications module state types and virtual-tree identity rules.
use fiex_storage::{native::NativeScopedStorage, CoreOwnerKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const URI: &str = "applications://";
pub const PROVIDER_ID: &str = "core:applications";
pub const SCHEMA: u32 = 1;
pub const FOLDER_SEGMENT: &str = "folder_";
pub const APP_SEGMENT: &str = "app_";
pub const STORAGE_SCOPE: &str = "tree";
pub const STATE_KEY: &str = "state";
const STATE_VALUE_LIMIT: usize = 16 * 1024 * 1024 - 1024;
mod assets;
pub use assets::{applications_root_path, imported_asset_path};
mod icon_cache;
pub use icon_cache::cached_icon_with;
mod catalog;
pub use catalog::{
    system_id, ApplicationCatalog, ApplicationDecorationInfo, ApplicationsBackend,
    CatalogApplication, InstalledApplication,
};
mod tree;
pub use tree::{parent_uri, validate_child_in};

/// Whether a launch target is an executable application format supported by
/// the Applications view. URL handlers and ordinary documents are excluded.
pub fn is_executable_target(value: &str) -> bool {
    if value.contains("://") {
        return false;
    }
    let extension = std::path::Path::new(value)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "exe" | "com" | "bat" | "cmd" | "msc" | "cpl" | "scr" | "ps1" | "appref-ms" | "jar"
    )
}

/// Stable storage identity shared by every provider registered by this module.
/// Keep its name, scope, key, and size quota aligned with the on-disk format.
pub fn owner_key() -> CoreOwnerKey {
    CoreOwnerKey::new("applications").with_value_limit(STORAGE_SCOPE, STATE_KEY, STATE_VALUE_LIMIT)
}

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

/// Owns the durable tree state format and its optimistic update rules.
#[derive(Clone)]
pub struct ApplicationsStore {
    storage: NativeScopedStorage,
}

impl ApplicationsStore {
    pub fn new(storage: NativeScopedStorage) -> Self {
        Self { storage }
    }

    pub fn snapshot(&self) -> Result<ApplicationsState, String> {
        let Some(value) = self.storage.get(STATE_KEY)? else {
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

    pub fn update<F>(&self, mut mutate: F) -> Result<ApplicationsState, String>
    where
        F: FnMut(&mut ApplicationsState) -> Result<(), String>,
    {
        self.storage
            .update(STATE_KEY, |current: Option<ApplicationsState>| {
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
}

pub fn validate_state(state: &ApplicationsState) -> Result<(), String> {
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
        let mut visited = HashSet::new();
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

pub fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

pub fn valid_extension(value: &str) -> bool {
    value.len() <= 16 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

pub fn folder_uri(id: &str) -> String {
    format!("{URI}{FOLDER_SEGMENT}{id}")
}

pub fn app_uri(id: &str) -> String {
    format!("{URI}{APP_SEGMENT}{id}")
}

pub fn folder_id(uri: &str) -> Option<String> {
    node_id(uri, FOLDER_SEGMENT)
}

pub fn app_id(uri: &str) -> Option<String> {
    node_id(uri, APP_SEGMENT)
}

fn node_id(uri: &str, leaf_prefix: &str) -> Option<String> {
    let relative = uri.strip_prefix(URI)?;
    let segments = relative.split('/');
    let mut last = None;
    for segment in segments {
        if segment.is_empty() {
            return None;
        }
        if let Some(previous) = last.replace(segment) {
            let parent_id = previous.strip_prefix(FOLDER_SEGMENT)?;
            if !valid_id(parent_id) {
                return None;
            }
        }
    }
    let id = last?.strip_prefix(leaf_prefix)?;
    valid_id(id).then(|| id.to_owned())
}

pub fn child_uri(parent_uri: &str, segment: &str) -> String {
    if parent_uri == URI {
        format!("{parent_uri}{segment}")
    } else {
        format!("{}/{segment}", parent_uri.trim_end_matches('/'))
    }
}

pub fn folder_uri_in(state: &ApplicationsState, id: &str) -> Option<String> {
    let mut segments = Vec::new();
    let mut current = state.folders.get(id)?;
    loop {
        segments.push(format!("{FOLDER_SEGMENT}{}", current.id));
        let Some(parent) = current.parent.as_deref() else {
            break;
        };
        current = state.folders.get(parent)?;
    }
    segments.reverse();
    Some(format!("{}{}", URI, segments.join("/")))
}

pub fn app_uri_in(state: &ApplicationsState, id: &str) -> Option<String> {
    let app = state.apps.get(id)?;
    let mut segments = Vec::new();
    let mut parent = app.parent.as_deref();
    while let Some(parent_id) = parent {
        let folder = state.folders.get(parent_id)?;
        segments.push(format!("{FOLDER_SEGMENT}{}", folder.id));
        parent = folder.parent.as_deref();
    }
    segments.reverse();
    segments.push(format!("{APP_SEGMENT}{id}"));
    Some(format!("{}{}", URI, segments.join("/")))
}

pub fn parent_uri_matches(state: &ApplicationsState, uri: &str) -> bool {
    if uri == URI {
        return true;
    }
    folder_id(uri).is_some_and(|id| folder_uri_in(state, &id).as_deref() == Some(uri))
}

#[cfg(test)]
mod tests;
