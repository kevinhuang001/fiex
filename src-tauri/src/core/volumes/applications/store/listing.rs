use super::super::{folders, list, system_entries, URI};
use super::path::{stored_path, stored_uri};
use crate::core::files::Entry;
use std::path::Path;

pub fn stored_entry(uri: &str) -> Result<Entry, String> {
    let path = stored_path(uri)?;
    let mut item = crate::core::files::entry(&path)?;
    item.path = uri.into();
    // Internal backing paths must not escape through metadata supplied to
    // extension providers. Launch and IO resolve the store URI inside Core.
    item.link_target = None;
    // Preserve the real content MIME for user-owned files: Preview, Archive,
    // Checksum and other Features must match exactly as on local storage.
    // Launch support is a separate application action, never a MIME override.
    item.allowed_ops = if item.is_dir {
        vec![
            "stat".into(),
            "list".into(),
            "search".into(),
            "mkdir".into(),
            "rename".into(),
            "remove".into(),
        ]
    } else {
        vec![
            "stat".into(),
            "read".into(),
            "write".into(),
            "rename".into(),
            "remove".into(),
        ]
    };
    Ok(item)
}

pub fn list_at(uri: &str) -> Result<Vec<Entry>, String> {
    if uri == URI {
        return list();
    }
    let entries = list_stored(&stored_path(uri)?)?;
    let mapped = folders::apps_in_folder(uri);
    if mapped.is_empty() {
        return Ok(entries);
    }
    Ok(folder_listing(entries, &system_entries()?, &mapped))
}

pub(in crate::core::volumes) fn folder_listing(
    mut entries: Vec<Entry>,
    system: &[Entry],
    mapped: &[String],
) -> Vec<Entry> {
    for app in mapped {
        let Some(entry) = system.iter().find(|entry| entry.path == *app).cloned() else {
            continue;
        };
        if !entries.iter().any(|candidate| candidate.path == entry.path) {
            entries.push(entry);
        }
    }
    entries
}

fn list_stored(dir: &Path) -> Result<Vec<Entry>, String> {
    let mut result = Vec::new();
    for item in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = item.map_err(|e| e.to_string())?.path();
        let uri = stored_uri(&path)?;
        // Escaping symlinks are omitted instead of granting host content.
        if let Ok(item) = stored_entry(&uri) {
            result.push(item);
        }
    }
    result.sort_by_key(|item| item.name.to_lowercase());
    Ok(result)
}
pub(in crate::core::volumes) fn list_user() -> Vec<Entry> {
    stored_path(URI)
        .ok()
        .and_then(|root| list_stored(&root).ok())
        .unwrap_or_default()
}
