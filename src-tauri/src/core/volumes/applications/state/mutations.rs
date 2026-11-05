use super::*;
use crate::core::files::Entry;

pub fn create_folder(parent_uri: &str, name: &str) -> Result<Entry, String> {
    crate::core::files::validate_name(name)?;
    let parent = folder_id_or_root(parent_uri)?;
    let id = random_id()?;
    let state = update(|state| {
        require_parent(state, parent.as_deref())?;
        if !parent_uri_matches(state, parent_uri) {
            return Err("StaleRef: Applications parent moved".into());
        }
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
    Ok(folder_entry(folder, &state))
}

pub fn rename_folder(uri: &str, name: &str) -> Result<Entry, String> {
    crate::core::files::validate_name(name)?;
    let id = folder_id(uri).ok_or("Applications root cannot be renamed")?;
    let state = update(|state| {
        if folder_uri_in(state, &id).as_deref() != Some(uri) {
            return Err("StaleRef: Applications folder moved".into());
        }
        let current = state
            .folders
            .get(&id)
            .cloned()
            .ok_or_else(|| "NotFound: application folder".to_string())?;
        ensure_unique_name(state, current.parent.as_deref(), name, Some(&id))?;
        state.folders.get_mut(&id).unwrap().name = name.to_owned();
        Ok(())
    })?;
    Ok(folder_entry(state.folders.get(&id).unwrap(), &state))
}

pub fn rename_imported(uri: &str, name: &str) -> Result<Entry, String> {
    crate::core::files::validate_name(name)?;
    let id = app_id(uri).ok_or("not an Applications app node")?;
    let state = update(|state| {
        if app_uri_in(state, &id).as_deref() != Some(uri) {
            return Err("StaleRef: Applications entry moved".into());
        }
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
    Ok(app_entry(state.apps.get(&id).unwrap(), &state))
}

pub fn rename_node(uri: &str, parent_uri: &str, name: &str) -> Result<Entry, String> {
    crate::core::files::validate_name(name)?;
    let target_parent = folder_id_or_root(parent_uri)?;
    if let Some(id) = folder_id(uri) {
        let state = update(|state| {
            if folder_uri_in(state, &id).as_deref() != Some(uri) {
                return Err("StaleRef: Applications folder moved".into());
            }
            if !parent_uri_matches(state, parent_uri) {
                return Err("StaleRef: Applications parent moved".into());
            }
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
        return Ok(folder_entry(state.folders.get(&id).unwrap(), &state));
    }
    let id = app_id(uri).ok_or("not an Applications node")?;
    let state = update(|state| {
        if app_uri_in(state, &id).as_deref() != Some(uri) {
            return Err("StaleRef: Applications entry moved".into());
        }
        if !parent_uri_matches(state, parent_uri) {
            return Err("StaleRef: Applications parent moved".into());
        }
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
    Ok(app_entry(state.apps.get(&id).unwrap(), &state))
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
        if folder_uri_in(&state, &id).as_deref() != Some(uri) {
            return Err("StaleRef: Applications folder moved".into());
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
        if folder_uri_in(state, &id).as_deref() != Some(uri) {
            return Err("StaleRef: Applications folder moved".into());
        }
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
    fiex_applications::parent_uri(uri)
}

pub fn validate_child(parent_uri: &str, child_uri: &str) -> Result<(), String> {
    if folder_id_or_root(parent_uri).is_err() {
        return Err("invalid Applications parent URI".into());
    }
    let state = snapshot()?;
    if !parent_uri_matches(&state, parent_uri) {
        return Err("StaleRef: Applications parent moved".into());
    }
    if let Some(pending) = pending_target(child_uri) {
        if pending.parent_uri == parent_uri {
            return Ok(());
        }
    }
    fiex_applications::validate_child_in(&state, parent_uri, child_uri)
}

pub fn move_nodes(uris: &[String], target_uri: &str) -> Result<Vec<String>, String> {
    migration::ensure_migrated()?;
    fiex_applications::ApplicationsStore::new(storage())
        .move_nodes(uris, target_uri)
        .map_err(|error| match error.as_str() {
            "同一目录中不能有同名项目" => {
                crate::i18n::t("同一目录中不能有同名项目").into()
            }
            "同名项目已存在，请更换名称" => {
                crate::i18n::t("同名项目已存在，请更换名称").into()
            }
            _ => error,
        })
}
