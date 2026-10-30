//! The platform Trash: locating it, recording where items came from so they can
//! be restored, and emptying it.

use crate::core::operations::check_cancelled;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

mod journal;
pub use journal::*;

/// The user's Trash can be browsed like any other folder. Windows exposes the
/// Recycle Bin through a shell namespace instead of a directory, so it keeps
/// the system opener as a fallback.
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
        Some(PathBuf::from(super::recycle::URI))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TrashOrigin {
    name: String,
    original: String,
}

pub fn restore_needs_folder(paths: &[String]) -> bool {
    paths.iter().any(|path| {
        !path.starts_with(super::recycle::URI)
            && super::trash_vfs::action_path(path)
                .ok()
                .and_then(|p| original_location(Path::new(&p)))
                .is_none()
    })
}

fn original_location(trashed: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    if let Some(original) = macos_put_back(trashed) {
        return Some(original);
    }
    let name = trashed.file_name()?.to_string_lossy().into_owned();
    // The freedesktop spec stores the original path next to the payload.
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
    load_journal()
        .into_iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.original)
}

fn free_destination(destination: PathBuf) -> PathBuf {
    if !destination.exists() {
        return destination;
    }
    let parent = destination
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let stem = destination
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| crate::i18n::t("恢复的项目").into());
    let extension = destination
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    for index in 2..10_000 {
        let candidate = parent.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    destination
}

fn move_entry(from: &Path, to: &Path) -> Result<(), String> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Cross-device restore: copy then remove.
    if from.is_dir() {
        copy_tree(from, to)?;
        std::fs::remove_dir_all(from).map_err(|e| e.to_string())
    } else {
        std::fs::copy(from, to).map_err(|e| e.to_string())?;
        std::fs::remove_file(from).map_err(|e| e.to_string())
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Finder records the previous location of a trashed item in `.Trash/.DS_Store`
/// (`ptbL`). Read it so Finder-trashed items can be put back from Fiex too.
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

/// Put trashed items back where they came from, like the system file manager.
pub fn restore_from_trash(
    paths: &[String],
    fallback: Option<&str>,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    let mut restored = 0;
    for path in paths {
        check_cancelled(cancel)?;
        let resolved = super::trash_vfs::action_path(path)?;
        let path = &resolved;
        if let Some(encoded) = path.strip_prefix(super::recycle::URI) {
            super::recycle::restore(encoded)?;
            restored += 1;
            continue;
        }
        let source = Path::new(path);
        if !source.exists() {
            continue;
        }
        let destination = original_location(source)
            .map(PathBuf::from)
            .or_else(|| {
                fallback
                    .and_then(|folder| source.file_name().map(|name| Path::new(folder).join(name)))
            })
            .ok_or_else(|| {
                crate::i18n::format("{path}: 请选择恢复位置", &[("path", path.to_string())])
            })?;
        if !destination.is_absolute() {
            return Err(crate::i18n::t("恢复位置必须为绝对路径").into());
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let destination = free_destination(destination);
        move_entry(source, &destination)?;
        if let Some(trash) = source.parent().and_then(|files| files.parent()) {
            if let Some(name) = source.file_name() {
                let _ = std::fs::remove_file(
                    trash
                        .join("info")
                        .join(format!("{}.trashinfo", name.to_string_lossy())),
                );
            }
        }
        let mut journal = load_journal();
        journal.retain(|entry| {
            source
                .file_name()
                .is_none_or(|name| name != entry.name.as_str())
        });
        save_journal(&journal);
        restored += 1;
    }
    Ok(restored)
}

fn remove_entry(path: &Path) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let result = if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|e| format!("{}: {e}", path.display()))
}

/// Permanently remove items. Used inside the Trash, where "move to Trash" has
/// no meaning and the user explicitly asks for the item to be gone.
pub fn delete_permanently(paths: &[String], cancel: &AtomicBool) -> Result<usize, String> {
    let mut removed = 0;
    for path in paths {
        check_cancelled(cancel)?;
        let resolved = super::trash_vfs::action_path(path)?;
        let path = &resolved;
        if let Some(encoded) = path.strip_prefix(super::recycle::URI) {
            super::recycle::purge(encoded)?;
            removed += 1;
            continue;
        }
        if super::applications::is_user(path) {
            super::applications::remove_user(path)?;
            removed += 1;
            continue;
        }
        if path.starts_with(super::applications::URI) {
            return Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into());
        }
        crate::plugins::mounts::reject_local(path)?;
        remove_entry(Path::new(path))?;
        removed += 1;
    }
    Ok(removed)
}

/// Empty the platform Trash the way the system file manager does.
pub fn empty_trash(cancel: &AtomicBool) -> Result<usize, String> {
    check_cancelled(cancel)?;
    #[cfg(target_os = "macos")]
    {
        let Some(trash) = trash_path() else {
            return Ok(0);
        };
        let mut removed = 0;
        for entry in std::fs::read_dir(&trash).map_err(|e| e.to_string())? {
            check_cancelled(cancel)?;
            let entry = entry.map_err(|e| e.to_string())?;
            remove_entry(&entry.path())?;
            removed += 1;
        }
        Ok(removed)
    }
    #[cfg(target_os = "linux")]
    {
        let data = dirs::data_dir().ok_or(crate::i18n::t("无法定位废纸篓"))?;
        let files = data.join("Trash/files");
        let info = data.join("Trash/info");
        let mut removed = 0;
        let entries = match std::fs::read_dir(&files) {
            Ok(entries) => entries,
            Err(_) => return Ok(0),
        };
        for entry in entries {
            check_cancelled(cancel)?;
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            remove_entry(&entry.path())?;
            // The freedesktop spec stores the original location next to the
            // payload as "<name>.trashinfo"; removing it keeps the Trash
            // consistent for other file managers.
            let _ = std::fs::remove_file(info.join(format!("{name}.trashinfo")));
            removed += 1;
        }
        Ok(removed)
    }
    #[cfg(target_os = "windows")]
    {
        super::recycle::empty()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(crate::i18n::t("当前系统请使用系统废纸篓清空").into())
    }
}
