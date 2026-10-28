use super::*;

pub(super) fn watchers() -> &'static std::sync::Mutex<Vec<notify::RecommendedWatcher>> {
    static WATCHERS: std::sync::OnceLock<std::sync::Mutex<Vec<notify::RecommendedWatcher>>> =
        std::sync::OnceLock::new();
    WATCHERS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Extracts a file to the local cache and watches it, writing edits back to the
/// raw filesystem whenever the external application saves.
pub fn open_cached(route: &Route) -> Result<String, String> {
    let data = read_path(route)?;
    let cache = dirs::home_dir()
        .ok_or(crate::i18n::t("无法创建缓存目录"))?
        .join(".fiex")
        .join("usbfs-cache")
        .join(Path::new(&route.root).file_name().unwrap_or_default())
        .join(&route.rel);
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&cache, data).map_err(|error| error.to_string())?;
    if matches!(route.kind, FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs) {
        return Ok(cache.to_string_lossy().into_owned());
    }
    fn digest(data: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }
    let watched = route.clone();
    let watched_file = cache.clone();
    let original = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(digest(&read_path(
        route,
    )?)));
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !matches!(
            event.kind,
            notify::EventKind::Modify(_) | notify::EventKind::Create(_)
        ) {
            return;
        }
        if !event.paths.iter().any(|path| path == &watched_file) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(220));
        let Ok(data) = std::fs::read(&watched_file) else {
            return;
        };
        let Ok(current) = read_path(&watched) else {
            return;
        };
        if digest(&current) != original.load(std::sync::atomic::Ordering::SeqCst) {
            let conflict = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".fiex")
                .join("usbfs-conflicts")
                .join(format!(
                    "{}-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|value| value.as_millis())
                        .unwrap_or_default(),
                    watched_file
                        .file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "file".into())
                ));
            if let Some(parent) = conflict.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(conflict, &data);
            return;
        }
        if write_path(&watched, &data).is_ok() {
            original.store(digest(&data), std::sync::atomic::Ordering::SeqCst);
        }
    })
    .map_err(|error| error.to_string())?;
    if let Some(parent) = cache.parent() {
        watcher
            .watch(parent, notify::RecursiveMode::NonRecursive)
            .map_err(|error| error.to_string())?;
    }
    watchers().lock().unwrap().push(watcher);
    Ok(cache.to_string_lossy().into_owned())
}
