use super::*;

pub(super) fn journal_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".fiex").join("trash-origins.json"))
}

pub(super) fn load_journal() -> Vec<TrashOrigin> {
    journal_path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub(super) fn save_journal(entries: &[TrashOrigin]) {
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
