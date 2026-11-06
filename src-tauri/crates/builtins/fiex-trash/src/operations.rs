use crate::{is_recycle_bin_uri, system_recycle_bin, system_trash_directory, RECYCLE_BIN_URI};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::journal;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::restore_fs::remove_entry;
use crate::restore_fs::{free_destination, move_entry};

const CANCELLED: &str = "Cancelled: user cancelled operation";

pub fn trash_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|home| home.join(".Trash"))
    }
    #[cfg(target_os = "linux")]
    {
        dirs::data_dir().map(|data| data.join("Trash/files"))
    }
    #[cfg(target_os = "windows")]
    {
        Some(PathBuf::from(RECYCLE_BIN_URI))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

pub fn move_to_trash(path: &Path) -> Result<(), String> {
    journal::move_to_trash(path)
}

pub fn restore_needs_folder(paths: &[String]) -> bool {
    paths.iter().any(|uri| {
        if is_recycle_bin_uri(uri) {
            return uri == RECYCLE_BIN_URI;
        }
        system_trash_directory()
            .physical(uri)
            .ok()
            .and_then(|path| original_location(&path))
            .is_none()
    })
}

pub fn restore_from_trash(
    paths: &[String],
    fallback: Option<&str>,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    journal::with_lock(|| restore_items(paths, fallback, cancel))
}

fn restore_items(
    paths: &[String],
    fallback: Option<&str>,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    let mut restored = 0;
    for uri in paths {
        check_cancelled(cancel)?;
        if uri.starts_with(RECYCLE_BIN_URI) {
            if uri == RECYCLE_BIN_URI {
                return Err("ScopeDenied: Recycle Bin root cannot be restored".into());
            }
            system_recycle_bin().restore(uri)?;
            restored += 1;
            continue;
        }
        if uri == crate::TRASH_URI {
            return Err("ScopeDenied: Trash root cannot be restored".into());
        }
        let source = system_trash_directory().physical(uri)?;
        match std::fs::symlink_metadata(&source) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.to_string()),
        }
        let destination = original_location(&source)
            .map(PathBuf::from)
            .or_else(|| {
                fallback
                    .and_then(|folder| source.file_name().map(|name| Path::new(folder).join(name)))
            })
            .ok_or_else(|| format!("{}: 请选择恢复位置", source.display()))?;
        if !destination.is_absolute() {
            return Err("恢复位置必须为绝对路径".into());
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let destination = free_destination(destination);
        move_entry(&source, &destination)?;
        remove_trash_metadata(&source);
        if let Some(root) = trash_path() {
            if source.parent() == Some(root.as_path()) {
                if let Some(name) = source.file_name() {
                    journal::forget_locked(&name.to_string_lossy());
                }
            }
        }
        restored += 1;
    }
    Ok(restored)
}

pub fn empty_trash(cancel: &AtomicBool) -> Result<usize, String> {
    check_cancelled(cancel)?;
    journal::with_lock(|| empty_trash_locked(cancel))
}

fn empty_trash_locked(cancel: &AtomicBool) -> Result<usize, String> {
    #[cfg(target_os = "windows")]
    {
        let _ = cancel;
        system_recycle_bin().empty()
    }
    #[cfg(target_os = "macos")]
    {
        let Some(trash) = trash_path() else {
            return Ok(0);
        };
        let removed = empty_directory(&trash, None, cancel)?;
        journal::clear_locked();
        Ok(removed)
    }
    #[cfg(target_os = "linux")]
    {
        let files = trash_path().ok_or("无法定位废纸篓")?;
        let info = files.parent().unwrap_or(&files).join("info");
        let removed = empty_directory(&files, Some(&info), cancel)?;
        journal::clear_locked();
        Ok(removed)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err("当前系统请使用系统废纸篓清空".into())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn empty_directory(
    directory: &Path,
    metadata: Option<&Path>,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.to_string()),
    };
    let mut removed = 0;
    for entry in entries {
        check_cancelled(cancel)?;
        let entry = entry.map_err(|error| error.to_string())?;
        remove_entry(&entry.path())?;
        if let Some(metadata) = metadata {
            let name = entry.file_name().to_string_lossy().into_owned();
            let _ = std::fs::remove_file(metadata.join(format!("{name}.trashinfo")));
        }
        removed += 1;
    }
    Ok(removed)
}

fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

fn original_location(trashed: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    if let Some(original) = macos_put_back(trashed) {
        return Some(original);
    }
    let name = trashed.file_name()?.to_string_lossy().into_owned();
    if let Some(trash) = trashed.parent().and_then(|files| files.parent()) {
        let info = trash.join("info").join(format!("{name}.trashinfo"));
        if let Ok(text) = std::fs::read_to_string(info) {
            for line in text.lines() {
                if let Some(value) = line.strip_prefix("Path=") {
                    if let Ok(decoded) = urlencoding::decode(value) {
                        if Path::new(decoded.as_ref()).is_absolute() {
                            return Some(decoded.into_owned());
                        }
                    }
                }
            }
        }
    }
    journal::original(&name)
}

#[cfg(target_os = "macos")]
fn macos_put_back(trashed: &Path) -> Option<String> {
    let name = trashed.file_name()?.to_string_lossy().into_owned();
    let bytes = std::fs::read(trashed.parent()?.join(".DS_Store")).ok()?;
    let store = ds_store::DsStore::new(&bytes).ok()?;
    let records = store.contents().get(&name)?;
    for key in ["ptbL", "ptbN"] {
        if let Some(ds_store::RecordValue::String(value)) = records.get(key) {
            if !value.is_empty() && Path::new(value).is_absolute() {
                return Some(value.clone());
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn remove_trash_metadata(source: &Path) {
    let Some(files) = trash_path() else {
        return;
    };
    if source.parent() != Some(files.as_path()) {
        return;
    }
    let Some(trash) = files.parent() else {
        return;
    };
    let Some(name) = source.file_name() else {
        return;
    };
    let _ = std::fs::remove_file(
        trash
            .join("info")
            .join(format!("{}.trashinfo", name.to_string_lossy())),
    );
}

#[cfg(not(target_os = "linux"))]
fn remove_trash_metadata(_: &Path) {}
