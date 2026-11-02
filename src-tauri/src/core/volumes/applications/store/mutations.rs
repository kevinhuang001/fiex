use super::super::{directory_cache, folders, URI};
use super::{
    listing::stored_entry,
    path::{is_user, stored_path, stored_uri},
};

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
    let app_bundle = target.is_dir()
        && target
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"));
    if app_bundle {
        std::fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        Ok(())
    } else if target.is_dir() {
        folders::remove_empty_folder(uri, &target)?;
        directory_cache::drop_subtree(uri);
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        Ok(())
    } else {
        std::fs::remove_file(target).map_err(|error| error.to_string())?;
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        Ok(())
    }
}
