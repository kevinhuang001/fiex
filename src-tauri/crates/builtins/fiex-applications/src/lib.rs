//! Applications module state types and virtual-tree identity rules.
use fiex_storage::{CoreOwnerKey, native::NativeScopedStorage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const URI: &str = "applications://";
pub const PROVIDER_ID: &str = "core:applications";
pub const SCHEMA: u32 = 1;
pub const STORAGE_SCOPE: &str = "tree";
pub const STATE_KEY: &str = "state";
const STATE_VALUE_LIMIT: usize = 16 * 1024 * 1024 - 1024;
mod assets;
pub use assets::{applications_root_path, imported_asset_path};
mod icon_cache;
pub use icon_cache::cached_icon_with;
mod catalog;
pub use catalog::{
    ApplicationCatalog, ApplicationsBackend, CatalogApplication, InstalledApplication, system_id,
};
mod tree;
pub use tree::{parent_uri, validate_child_in};
mod presentation;
pub use presentation::ApplicationDecorationInfo;
mod target;
pub use target::is_executable_target;
mod uri;
pub use uri::{app_id, app_uri_in, child_uri, folder_id, folder_uri_in, parent_uri_matches};

/// Stable storage identity shared by every provider registered by this module.
/// Keep its name, scope, key, and size quota aligned with the on-disk format.
pub fn owner_key() -> CoreOwnerKey {
    CoreOwnerKey::new("applications").with_value_limit(STORAGE_SCOPE, STATE_KEY, STATE_VALUE_LIMIT)
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationsState {
    pub schema: u32,
    pub revision: u64,
    pub folders: BTreeMap<String, VirtualFolder>,
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
#[serde(deny_unknown_fields)]
pub struct ApplicationNode {
    pub id: String,
    pub parent: Option<String>,
    pub display_name: String,
    pub source: ApplicationSource,
    pub order: i64,
    pub unavailable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ApplicationSource {
    System { stable_locator: String },
    Imported { asset_id: String, extension: String },
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
    pub id: String,
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
    let mut sibling_names = HashSet::new();
    for (id, folder) in &state.folders {
        if id != &folder.id || !valid_id(id) || !valid_name(&folder.name) {
            return Err("invalid Applications folder state".into());
        }
        if !sibling_names.insert((folder.parent.clone(), folder.name.to_lowercase())) {
            return Err("duplicate Applications sibling name".into());
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
        if id != &app.id || !valid_id(id) || !valid_name(&app.display_name) {
            return Err("invalid Applications entry state".into());
        }
        if !sibling_names.insert((app.parent.clone(), app.display_name.to_lowercase())) {
            return Err("duplicate Applications sibling name".into());
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

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value.contains('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
}

pub fn valid_extension(value: &str) -> bool {
    value.len() <= 16 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests;
