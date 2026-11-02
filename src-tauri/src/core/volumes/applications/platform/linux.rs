use super::super::{decode, encode, entry, URI};
use crate::core::files::Entry;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
pub(super) fn launch_desktop(path: &Path) -> Result<(), String> {
    if std::process::Command::new("gio")
        .args(["launch"])
        .arg(path)
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(());
    }
    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
        if std::process::Command::new("gtk-launch")
            .arg(stem)
            .status()
            .is_ok_and(|status| status.success())
        {
            return Ok(());
        }
    }
    Err(crate::i18n::t("无法启动应用").into())
}

#[cfg(target_os = "linux")]
pub(in crate::core::volumes::applications) fn platform_list() -> Result<Vec<Entry>, String> {
    let mut roots = Vec::new();
    if let Some(data) = dirs::data_dir() {
        roots.push(data.join("applications"));
    }
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for base in data_dirs.split(':').filter(|value| !value.is_empty()) {
        roots.push(Path::new(base).join("applications"));
    }
    roots.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    if let Some(data) = dirs::data_dir() {
        roots.push(data.join("flatpak/exports/share/applications"));
    }
    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        collect_desktop_files(&root, &mut entries, &mut seen, 0)?;
    }
    entries.sort_by_key(|left| left.name.to_lowercase());
    Ok(entries)
}

#[cfg(target_os = "linux")]
fn collect_desktop_files(
    dir: &Path,
    entries: &mut Vec<Entry>,
    seen: &mut std::collections::HashSet<String>,
    depth: u8,
) -> Result<(), String> {
    if depth > 3 {
        return Ok(());
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for item in read.flatten() {
        let path = item.path();
        if path.is_dir() {
            let _ = collect_desktop_files(&path, entries, seen, depth + 1);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("desktop") {
            continue;
        }
        let Some((name, hidden, is_app)) = desktop_entry(&path) else {
            continue;
        };
        if hidden || !is_app || !seen.insert(name.clone()) {
            continue;
        }
        let modified = item
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let encoded = encode(&path.to_string_lossy());
        let mut item = entry(name, format!("{URI}{encoded}"), "desktop", modified, true);
        item.link_target = Some(path.to_string_lossy().into_owned());
        entries.push(item);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(in crate::core::volumes::applications) fn desktop_entry(
    path: &Path,
) -> Option<(String, bool, bool)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut section = String::new();
    let mut name = None;
    let mut hidden = false;
    let mut is_app = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.to_string();
            continue;
        }
        if section != "[Desktop Entry]" {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "Name" => name = Some(value.trim().to_string()),
            "Hidden" => hidden = value.trim().eq_ignore_ascii_case("true"),
            "NoDisplay" => hidden |= value.trim().eq_ignore_ascii_case("true"),
            "Type" => is_app = value.trim() == "Application",
            _ => {}
        }
    }
    let name = name.filter(|value| !value.is_empty())?;
    Some((name, hidden, is_app))
}

#[cfg(target_os = "linux")]
pub(in crate::core::volumes::applications) fn platform_open(encoded: &str) -> Result<(), String> {
    let path = decode(encoded)?;
    let path = Path::new(&path);
    // gio launches by .desktop path without starting a file manager; fall back
    // to GTK's launcher on systems where gio is not installed.
    if std::process::Command::new("gio")
        .args(["launch"])
        .arg(path)
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(());
    }
    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
        if std::process::Command::new("gtk-launch")
            .arg(stem)
            .status()
            .is_ok_and(|status| status.success())
        {
            return Ok(());
        }
    }
    Err(crate::i18n::t("无法启动应用").into())
}
