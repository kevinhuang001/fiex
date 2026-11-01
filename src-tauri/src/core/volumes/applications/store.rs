//! Persistent user-created items stored under the private Applications directory.
use super::{application_folders, directory_cache, list, system_entries, system_entry, URI, USER_PREFIX};
use crate::core::files::Entry;
use std::path::{Path, PathBuf};

pub(super) fn user_encoded(uri: &str) -> Option<&str> {
    uri.strip_prefix(USER_PREFIX)
}

pub fn is_user(path: &str) -> bool {
    user_encoded(path).is_some()
}

fn user_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".fiex")
        .join("applications")
}

/// The writable side of Applications is a sandboxed local store. URI payloads
/// are relative to that store, including nested user-created directories.
pub fn stored_path(uri: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(user_dir()).map_err(|e| e.to_string())?;
    let root = dunce::canonicalize(user_dir()).map_err(|e| e.to_string())?;
    if uri == URI {
        return Ok(root);
    }
    let relative = decode(user_encoded(uri).ok_or("not a stored application")?)?;
    if relative.is_empty()
        || relative.contains(['\\', ':', '\0'])
        || Path::new(&relative)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("invalid application store path".into());
    }
    let target = root.join(relative);
    let canonical = dunce::canonicalize(&target).map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root) || canonical == root {
        return Err("application store escape".into());
    }
    Ok(canonical)
}

/// Validate an Applications user URI without requiring the final node to
/// exist. A newly created child is confined to the canonical private root;
/// every existing ancestor is canonicalized to reject symlink escapes.
pub fn candidate_path(uri: &str) -> Result<PathBuf, String> {
    let root = stored_path(URI)?;
    let encoded = user_encoded(uri).ok_or("not a user application")?;
    let relative = decode(encoded)?;
    if relative.is_empty()
        || relative.contains(['\\', ':', '\0'])
        || Path::new(&relative)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("invalid application store path".into());
    }
    let target = root.join(relative);
    let mut ancestor = target.as_path();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(
            ancestor
                .file_name()
                .ok_or("invalid application child path")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().ok_or("missing application ancestor")?;
    }
    let mut resolved = dunce::canonicalize(ancestor).map_err(|e| e.to_string())?;
    if !resolved.starts_with(&root) {
        return Err("application store escape".into());
    }
    for child in missing.iter().rev() {
        resolved.push(child);
    }
    if resolved == root {
        return Err("invalid application child path".into());
    }
    Ok(resolved)
}

/// Ensure a generated child belongs directly to the selected Applications
/// directory. This is used when Core binds a provider-returned ResourceRef.
pub fn validate_child(parent: &str, child: &str) -> Result<(), String> {
    let parent_path = if parent == URI {
        stored_path(parent)?
    } else if is_user(parent) {
        candidate_path(parent)?
    } else {
        return Err("ScopeDenied: invalid application parent".into());
    };
    let child_path = candidate_path(child)?;
    if child_path.parent() != Some(parent_path.as_path()) {
        return Err("ScopeDenied: child outside selected application directory".into());
    }
    Ok(())
}

pub fn stored_uri(path: &Path) -> Result<String, String> {
    let root = dunce::canonicalize(user_dir()).map_err(|e| e.to_string())?;
    let rel = path
        .strip_prefix(root)
        .map_err(|_| "not in application store")?;
    Ok(format!(
        "{USER_PREFIX}{}",
        encode(&rel.to_string_lossy().replace('\\', "/"))
    ))
}

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
    let mapped = application_folders::apps_in_folder(uri);
    if mapped.is_empty() {
        return Ok(entries);
    }
    Ok(folder_listing(entries, &system_entries()?, &mapped))
}

pub(super) fn folder_listing(
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
pub(super) fn list_user() -> Vec<Entry> {
    stored_path(URI)
        .ok()
        .and_then(|root| list_stored(&root).ok())
        .unwrap_or_default()
}

pub fn create_stored(parent: &str, name: &str, folder: bool) -> Result<String, String> {
    crate::core::files::validate_name(name)?;
    let target = stored_path(parent)?.join(name);
    if folder {
        std::fs::create_dir(&target).map_err(|e| e.to_string())?;
    } else {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| e.to_string())?;
    }
    let uri = stored_uri(&target)?;
    if let Ok(entry) = stored_entry(&uri) {
        directory_cache::add_entry(parent, entry);
    }
    Ok(uri)
}

pub fn rename_stored(uri: &str, name: &str) -> Result<String, String> {
    crate::core::files::validate_name(name)?;
    let source = stored_path(uri)?;
    let target = source.with_file_name(name);
    if target.exists() {
        return Err(crate::i18n::t("同名项目已存在，请更换名称").into());
    }
    let directory = source.is_dir();
    std::fs::rename(&source, &target).map_err(|e| e.to_string())?;
    let renamed = stored_uri(&target)?;
    after_rename(uri, &renamed, directory)?;
    Ok(renamed)
}

/// Keep persisted folder mappings and the parent cache coherent after a folder
/// rename. The native VFS provider calls this after its rename commit, because it renames the
/// backing store directly.
pub fn after_rename(old_uri: &str, new_uri: &str, directory: bool) -> Result<(), String> {
    if directory {
        application_folders::rename_folder(old_uri, new_uri)?;
        directory_cache::drop_subtree(old_uri);
    }
    let renamed = stored_path(new_uri)?;
    let parent = renamed.parent().ok_or("application item has no parent")?;
    let root = stored_path(URI)?;
    let parent_uri = if parent == root {
        URI.to_string()
    } else {
        stored_uri(parent)?
    };
    if let Ok(entry) = stored_entry(new_uri) {
        directory_cache::rename_entry(&parent_uri, old_uri, entry);
    }
    Ok(())
}

pub fn remove_user(uri: &str) -> Result<(), String> {
    if !is_user(uri) {
        return Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into());
    }
    let target = stored_path(uri)?;
    let root = stored_path(URI)?;
    let parent = target.parent().ok_or("application item has no parent")?;
    let parent_uri = if parent == root {
        URI.to_string()
    } else {
        stored_uri(parent)?
    };
    if target.is_dir() {
        // Deleting an application folder is non-destructive for applications:
        // real children return to the root and mapped system apps follow.
        let mut moved = Vec::new();
        let children = std::fs::read_dir(&target)
            .map_err(|error| error.to_string())?
            .map(|item| item.map(|item| item.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for child in children {
            let name = child.file_name().ok_or("application child has no name")?;
            let destination = unique_child(&root, name);
            std::fs::rename(&child, &destination).map_err(|error| error.to_string())?;
            if let Ok(entry) = stored_entry(&stored_uri(&destination)?) {
                moved.push(entry);
            }
        }
        let returned = application_folders::forget_folder(uri)?;
        std::fs::remove_dir(&target).map_err(|error| error.to_string())?;
        directory_cache::drop_subtree(uri);
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        for entry in moved {
            directory_cache::add_entry(URI, entry);
        }
        for app in returned {
            if let Ok(entry) = system_entry(&app) {
                directory_cache::add_entry(URI, entry);
            }
        }
        Ok(())
    } else {
        std::fs::remove_file(target).map_err(|error| error.to_string())?;
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        Ok(())
    }
}

fn unique_child(root: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let mut target = root.join(name);
    if !target.exists() {
        return target;
    }
    let name = name.to_string_lossy();
    let path = Path::new(name.as_ref());
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("application");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let mut index = 2;
    loop {
        target = root.join(format!("{stem} ({index}){extension}"));
        if !target.exists() {
            return target;
        }
        index += 1;
    }
}

