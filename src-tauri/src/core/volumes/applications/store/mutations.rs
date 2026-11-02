use super::super::{directory_cache, folders, URI};
use super::{
    listing::stored_entry,
    path::{is_user, lstat_path, stored_path, stored_uri},
};
use std::path::{Path, PathBuf};

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
        folders::rename_folder(old_uri, new_uri)?;
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
    let target = validate_user_removal(uri)?;
    let root = stored_path(URI)?;
    let parent = target.parent().ok_or("application item has no parent")?;
    let parent_uri = if parent == root {
        URI.to_string()
    } else {
        stored_uri(parent)?
    };
    let metadata = std::fs::symlink_metadata(&target).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        std::fs::remove_file(&target).map_err(|error| error.to_string())?;
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        Ok(())
    } else if metadata.is_dir() {
        std::fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
        let _ = folders::forget_folder(uri)?;
        directory_cache::drop_subtree(uri);
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        Ok(())
    } else {
        std::fs::remove_file(&target).map_err(|error| error.to_string())?;
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        Ok(())
    }
}

/// Validate a user deletion against the current real tree and persisted
/// system-app mappings. Symlink ancestors are rejected and recursive scans do
/// not follow symlinks.
pub(in crate::core::volumes::applications) fn validate_user_removal(
    uri: &str,
) -> Result<PathBuf, String> {
    if !is_user(uri) {
        return Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into());
    }
    let target = lstat_path(uri)?;
    // Resolve once to reject stale paths and any link that escapes the private
    // store, while retaining `target` so the final node is inspected by lstat.
    let _ = stored_path(uri)?;
    let root = stored_path(URI)?;
    let relative = target
        .strip_prefix(&root)
        .map_err(|_| "application store escape")?;
    let mut current = root.clone();
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        if index + 1 < components.len()
            && std::fs::symlink_metadata(&current)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err("application folder contains a symlinked ancestor".into());
        }
    }
    let metadata = std::fs::symlink_metadata(&target).map_err(|error| error.to_string())?;
    if metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && !super::super::platform::is_user_application(&target)
        && (folders::contains_mapped_apps(uri) || contains_application(&target)?)
    {
        return Err(crate::i18n::t("此文件夹包含应用，移出应用后才能删除").into());
    }
    Ok(target)
}

fn contains_application(directory: &Path) -> Result<bool, String> {
    for child in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
        let child = child.map_err(|error| error.to_string())?.path();
        let metadata = std::fs::symlink_metadata(&child).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if super::super::platform::is_user_application(&child) {
            return Ok(true);
        }
        if metadata.is_dir() && contains_application(&child)? {
            return Ok(true);
        }
    }
    Ok(false)
}
