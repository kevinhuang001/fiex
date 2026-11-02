use super::super::{directory_cache, folders, system_entry, URI};
use super::{
    listing::stored_entry,
    path::{is_user, stored_path, stored_uri},
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
        let returned = folders::forget_folder(uri)?;
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
