use super::*;
use crate::core::files::Entry;

pub fn folder_entry(folder: &VirtualFolder, state: &ApplicationsState) -> Entry {
    let uri = folder_uri_in(state, &folder.id).expect("folder is part of the validated tree");
    // Remove is a supported operation for virtual folders. Whether a folder
    // is empty is checked by the Provider's remove preflight at execution.
    let operations = [
        "stat", "list", "search", "write", "mkdir", "rename", "remove",
    ];
    application_entry(folder.name.clone(), uri, true, &operations)
}

pub fn app_entry(app: &ApplicationNode, state: &ApplicationsState) -> Entry {
    let operations: &[&str] = match &app.source {
        ApplicationSource::System { .. } => &["stat"],
        ApplicationSource::Imported { .. } => &["stat", "read", "remove", "rename"],
    };
    let mut entry = application_entry(
        app.display_name.clone(),
        app_uri_in(state, &app.id).expect("application is part of the validated tree"),
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

pub fn folder_children(uri: &str) -> Result<Vec<Entry>, String> {
    let state = snapshot()?;
    let parent = if uri == super::super::URI {
        None
    } else {
        Some(folder_id(&state, uri).ok_or("invalid Applications folder URI")?)
    };
    if !parent_uri_matches(&state, uri) {
        return Err("StaleRef: Applications folder moved".into());
    }
    if let Some(id) = parent.as_deref() {
        require_parent(&state, Some(id))?;
    }
    let mut entries = state
        .folders
        .values()
        .filter(|folder| folder.parent == parent)
        .map(|folder| folder_entry(folder, &state))
        .chain(
            state
                .apps
                .values()
                .filter(|app| app.parent == parent)
                .map(|app| app_entry(app, &state)),
        )
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.name.to_lowercase());
    Ok(entries)
}

pub fn entry(uri: &str) -> Result<Entry, String> {
    if uri == super::super::URI {
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
    if let Some(id) = folder_id(&state, uri) {
        let entry = state
            .folders
            .get(&id)
            .map(|folder| folder_entry(folder, &state))
            .ok_or_else(|| "NotFound: application folder".to_string())?;
        if entry.path != uri {
            return Err("StaleRef: Applications folder moved".into());
        }
        return Ok(entry);
    }
    if let Some(id) = app_id(&state, uri) {
        let entry = state
            .apps
            .get(&id)
            .map(|app| app_entry(app, &state))
            .ok_or_else(|| "NotFound: application".to_string())?;
        if entry.path != uri {
            return Err("StaleRef: Applications entry moved".into());
        }
        return Ok(entry);
    }
    Err("invalid Applications URI".into())
}

pub fn source(uri: &str) -> Result<ApplicationNode, String> {
    let state = snapshot()?;
    let id = app_id(&state, uri).ok_or("not an Applications app node")?;
    if app_uri_in(&state, &id).as_deref() != Some(uri) {
        return Err("StaleRef: Applications entry moved".into());
    }
    state
        .apps
        .get(&id)
        .cloned()
        .ok_or_else(|| "NotFound: application".into())
}
