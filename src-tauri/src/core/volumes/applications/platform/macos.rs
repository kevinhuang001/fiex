use super::super::{URI, decode, encode, entry};
use crate::core::files::Entry;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub(in crate::core::volumes::applications) fn platform_list() -> Result<Vec<Entry>, String> {
    let mut dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("Applications"));
    }
    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            let path = item.path();
            if path.extension().and_then(|value| value.to_str()) != Some("app") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            if !seen.insert(name.to_string()) {
                continue;
            }
            let modified = item
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let mut item = entry(
                name.to_string(),
                format!("{URI}{}", encode(&path.to_string_lossy())),
                "app",
                modified,
            );
            item.link_target = Some(path.to_string_lossy().into_owned());
            entries.push(item);
        }
    }
    entries.sort_by_key(|left| left.name.to_lowercase());
    Ok(entries)
}

#[cfg(target_os = "macos")]
pub(in crate::core::volumes::applications) fn platform_open(path: &str) -> Result<(), String> {
    // Applications are real bundles here; the system opener launches the app
    // without revealing a Finder window.
    crate::core::terminals::open_path(&decode(path)?)
}
