//! The platform Trash: locating it, recording where items came from so they can
//! be restored, and emptying it.

use crate::operations::check_cancelled;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// The user's Trash can be browsed like any other folder. Windows exposes the
/// Recycle Bin through a shell namespace instead of a directory, so it keeps
/// the system opener as a fallback.
pub fn trash_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|home| home.join(".Trash"))
            .filter(|path| path.is_dir())
    }
    #[cfg(target_os = "linux")]
    {
        dirs::data_dir()
            .map(|data| data.join("Trash/files"))
            .filter(|path| path.is_dir())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TrashOrigin {
    name: String,
    original: String,
}

fn journal_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".fiex").join("trash-origins.json"))
}

fn load_journal() -> Vec<TrashOrigin> {
    journal_path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_journal(entries: &[TrashOrigin]) {
    let Some(path) = journal_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(entries) {
        let _ = std::fs::write(path, bytes);
    }
}

/// Capture actual trash names so renamed collisions retain their own origin.
pub fn trash_names() -> HashSet<String> {
    trash_path()
        .and_then(|path| std::fs::read_dir(path).ok())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

pub fn record_trashed(original: &str, before: &HashSet<String>) {
    let after = trash_names();
    let added: Vec<_> = after.difference(before).collect();
    // Never guess when another process concurrently puts an item in the trash.
    if added.len() != 1 {
        return;
    }
    let name = added[0].clone();
    let mut entries = load_journal();
    entries.retain(|entry| entry.name != name && after.contains(&entry.name));
    entries.push(TrashOrigin {
        name,
        original: original.into(),
    });
    save_journal(&entries);
}

pub fn restore_needs_folder(paths: &[String]) -> bool {
    paths
        .iter()
        .any(|path| original_location(Path::new(path)).is_none())
}

fn original_location(trashed: &Path) -> Option<String> {
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
        .unwrap_or_else(|| "恢复的项目".into());
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

/// Put trashed items back where they came from, like the system file manager.
pub fn restore_from_trash(
    paths: &[String],
    fallback: Option<&str>,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    let mut restored = 0;
    for path in paths {
        check_cancelled(cancel)?;
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
            .ok_or_else(|| format!("{path}: 请选择恢复位置"))?;
        if !destination.is_absolute() {
            return Err("恢复位置必须为绝对路径".into());
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
        let data = dirs::data_dir().ok_or("无法定位废纸篓")?;
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
        let mut command = std::process::Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Clear-RecycleBin -Force -ErrorAction SilentlyContinue",
        ]);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let status = command.status().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(0)
        } else {
            Err("无法清空回收站".into())
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err("当前系统请使用系统废纸篓清空".into())
    }
}
