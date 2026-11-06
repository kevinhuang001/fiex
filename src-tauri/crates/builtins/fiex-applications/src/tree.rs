//! Applications virtual-tree relationship and relocation rules.
use crate::{
    app_id, app_uri_in, folder_id, folder_uri_in, parent_uri_matches, ApplicationsState,
    ApplicationsStore, URI,
};
use std::collections::HashSet;

impl ApplicationsStore {
    /// Move tree nodes by changing their persisted parent and order only.
    pub fn move_nodes(&self, uris: &[String], target_uri: &str) -> Result<Vec<String>, String> {
        let mut moved = Vec::new();
        self.update(|state| {
            moved = move_nodes_in_state(state, uris, target_uri)?;
            Ok(())
        })?;
        Ok(moved)
    }
}

/// Resolve the parent path from the VFS hierarchy.
pub fn parent_uri(uri: &str) -> Result<String, String> {
    let relative = uri
        .strip_prefix(URI)
        .ok_or("invalid Applications node URI")?;
    if relative.is_empty() || relative.ends_with('/') {
        return Err("invalid Applications node URI".into());
    }
    let separator = relative.rfind('/');
    Ok(separator
        .map(|index| format!("{URI}{}", &relative[..index]))
        .unwrap_or_else(|| URI.into()))
}

/// Verify that the persisted node is a canonical child of the requested folder.
pub fn validate_child_in(
    state: &ApplicationsState,
    parent_uri: &str,
    child_uri: &str,
) -> Result<(), String> {
    let expected_parent = folder_id_or_root(state, parent_uri)?;
    if !parent_uri_matches(state, parent_uri) {
        return Err("StaleRef: Applications parent moved".into());
    }
    if let Some(id) = folder_id(state, child_uri) {
        let folder = state.folders.get(&id);
        if folder.and_then(|item| item.parent.as_deref()) == expected_parent.as_deref()
            && folder_uri_in(state, &id).as_deref() == Some(child_uri)
        {
            return Ok(());
        }
    } else if let Some(id) = app_id(state, child_uri) {
        let app = state.apps.get(&id);
        if app.and_then(|item| item.parent.as_deref()) == expected_parent.as_deref()
            && app_uri_in(state, &id).as_deref() == Some(child_uri)
        {
            return Ok(());
        }
    }
    Err("InvalidVfsUri: application is not a child of this folder".into())
}

fn move_nodes_in_state(
    state: &mut ApplicationsState,
    uris: &[String],
    target_uri: &str,
) -> Result<Vec<String>, String> {
    let target = folder_id_or_root(state, target_uri)?;
    let mut resolved = Vec::new();
    for uri in uris {
        if let Some(id) = folder_id(state, uri) {
            resolved.push((id, true));
        } else if let Some(id) = app_id(state, uri) {
            resolved.push((id, false));
        } else {
            return Err("invalid Applications node".into());
        }
    }
    let mut ids = resolved.clone();
    ids.sort();
    ids.dedup();
    require_parent(state, target.as_deref())?;
    if !parent_uri_matches(state, target_uri) {
        return Err("StaleRef: Applications destination moved".into());
    }
    for uri in uris {
        let current = folder_id(state, uri)
            .and_then(|id| folder_uri_in(state, &id))
            .or_else(|| app_id(state, uri).and_then(|id| app_uri_in(state, &id)));
        if current.as_deref() != Some(uri) {
            return Err("StaleRef: Applications item moved".into());
        }
    }

    let moving_folders = ids
        .iter()
        .filter_map(|(id, is_folder)| is_folder.then_some(id.as_str()))
        .collect::<HashSet<_>>();
    let moving_apps = ids
        .iter()
        .filter_map(|(id, is_folder)| (!is_folder).then_some(id.as_str()))
        .collect::<HashSet<_>>();
    let mut names = HashSet::new();
    for (id, is_folder) in &ids {
        if *is_folder {
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
            if !names.insert(node.name.to_lowercase()) {
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
                    .and_then(|folder| folder.parent.as_deref());
            }
        } else {
            let node = state
                .apps
                .get(id)
                .ok_or_else(|| "NotFound: application".to_string())?;
            if node.parent == target {
                continue;
            }
            if !names.insert(node.display_name.to_lowercase()) {
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

    for (id, is_folder) in &ids {
        if *is_folder {
            if state
                .folders
                .get(id)
                .is_some_and(|node| node.parent == target)
            {
                continue;
            }
            let order = next_order(state, target.as_deref());
            let node = state.folders.get_mut(id).expect("validated folder ID");
            node.parent = target.clone();
            node.order = order;
        } else {
            if state.apps.get(id).is_some_and(|node| node.parent == target) {
                continue;
            }
            let order = next_order(state, target.as_deref());
            let node = state.apps.get_mut(id).expect("validated application ID");
            node.parent = target.clone();
            node.order = order;
        }
    }

    resolved
        .iter()
        .map(|(id, is_folder)| {
            if *is_folder {
                return folder_uri_in(state, &id)
                    .ok_or_else(|| "NotFound: application folder".into());
            }
            app_uri_in(state, &id).ok_or_else(|| "NotFound: application".into())
        })
        .collect()
}

fn folder_id_or_root(state: &ApplicationsState, uri: &str) -> Result<Option<String>, String> {
    if uri == URI {
        Ok(None)
    } else {
        folder_id(state, uri)
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

fn ensure_name_available_except(
    state: &ApplicationsState,
    parent: Option<&str>,
    name: &str,
    moving_folders: &HashSet<&str>,
    moving_apps: &HashSet<&str>,
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
        return Err("同名项目已存在，请更换名称".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "tree/tests.rs"]
mod tests;
