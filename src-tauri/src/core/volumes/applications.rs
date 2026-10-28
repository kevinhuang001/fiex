//! The "Applications" virtual folder. Core enumerates installed applications
//! itself so opening the entry never has to launch Finder or Explorer.

use crate::core::files::Entry;
use std::path::{Path, PathBuf};

pub const URI: &str = "applications://";
pub const USER_PREFIX: &str = "applications://user/";

pub fn is_virtual(path: &str) -> bool {
    path == URI
}

fn entry(name: String, path: String, extension: &str, modified: u64, readonly: bool) -> Entry {
    Entry {
        path,
        name,
        is_dir: false,
        is_symlink: false,
        size: 0,
        modified,
        created: modified,
        extension: extension.into(),
        mime: "application/x-fiex-application".into(),
        readonly,
        link_target: None,
        allowed_ops: vec![],
        tags: vec![],
        has_cache: false,
        pinned: false,
        uploaded: false,
        sync: None,
    }
}

pub fn list() -> Result<Vec<Entry>, String> {
    let mut entries = platform_list()?;
    entries.extend(list_user());
    Ok(entries)
}

pub fn open(encoded: &str) -> Result<(), String> {
    if let Some(name) = encoded.strip_prefix("user/") {
        let name = decode(name)?;
        let path = user_dir().join(name);
        return open_stored(&path);
    }
    platform_open(encoded)
}

pub fn is_user(path: &str) -> bool {
    path.starts_with(USER_PREFIX)
}

fn user_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".fiex")
        .join("applications")
}

fn list_user() -> Vec<Entry> {
    let dir = user_dir();
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for item in read.flatten() {
        let path = item.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_lowercase();
        let modified = item
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        entries.push(entry(
            name.to_string(),
            format!("{USER_PREFIX}{}", encode(name)),
            if extension.is_empty() {
                "app"
            } else {
                &extension
            },
            modified,
            false,
        ));
    }
    entries.sort_by_key(|left| left.name.to_lowercase());
    entries
}

/// Copy dragged shortcuts/app bundles into the user store. Dragging a system
/// application never removes the original; only Fiex-owned items are copied.
pub fn import(paths: &[String]) -> crate::core::operations::OperationResult {
    let dir = user_dir();
    let mut result = crate::core::operations::OperationResult {
        paths: Vec::new(),
        errors: Vec::new(),
        skipped: 0,
        conflicts: Vec::new(),
    };
    if let Err(error) = std::fs::create_dir_all(&dir) {
        result.errors.push(error.to_string());
        return result;
    }
    for source in paths {
        let source_path = Path::new(source);
        let Some(name) = source_path.file_name().and_then(|value| value.to_str()) else {
            result.errors.push(format!("{source}: invalid name"));
            continue;
        };
        let mut destination = dir.join(name);
        let mut index = 2;
        while destination.exists() {
            let stem = source_path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("item");
            let extension = source_path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| format!(".{value}"))
                .unwrap_or_default();
            destination = dir.join(format!("{stem} ({index}){extension}"));
            index += 1;
        }
        let copied = if source_path.is_dir() {
            copy_tree(source_path, &destination)
        } else {
            std::fs::copy(source_path, &destination)
                .map(|_| ())
                .map_err(|error| error.to_string())
        };
        match copied {
            Ok(()) => result
                .paths
                .push(destination.to_string_lossy().into_owned()),
            Err(error) => result.errors.push(format!("{source}: {error}")),
        }
    }
    result
}

pub fn remove_user(path: &str) -> Result<(), String> {
    let Some(encoded) = path.strip_prefix(USER_PREFIX) else {
        return Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into());
    };
    let name = decode(encoded)?;
    let target = user_dir().join(name);
    if target.is_dir() {
        std::fs::remove_dir_all(&target).map_err(|error| error.to_string())
    } else {
        std::fs::remove_file(&target).map_err(|error| error.to_string())
    }
}

fn open_stored(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return crate::core::terminals::open_path(&path.to_string_lossy());
    }
    #[cfg(target_os = "linux")]
    {
        if path.extension().and_then(|value| value.to_str()) == Some("desktop") {
            return launch_desktop(path);
        }
        return open::that_detached(path).map_err(|error| error.to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .status()
            .map_err(|error| error.to_string())?;
        return if status.success() {
            Ok(())
        } else {
            Err(crate::i18n::t("无法启动应用").into())
        };
    }
    #[allow(unreachable_code)]
    Err(crate::i18n::t("无法启动应用").into())
}

#[cfg(target_os = "linux")]
fn launch_desktop(path: &Path) -> Result<(), String> {
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

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|error| error.to_string())?;
    for item in std::fs::read_dir(from).map_err(|error| error.to_string())? {
        let item = item.map_err(|error| error.to_string())?;
        let target = to.join(item.file_name());
        if item.path().is_dir() {
            copy_tree(&item.path(), &target)?;
        } else {
            std::fs::copy(item.path(), &target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_list() -> Result<Vec<Entry>, String> {
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
                path.to_string_lossy().into_owned(),
                "app",
                modified,
                false,
            );
            item.link_target = Some(path.to_string_lossy().into_owned());
            entries.push(item);
        }
    }
    entries.sort_by_key(|left| left.name.to_lowercase());
    Ok(entries)
}

#[cfg(target_os = "macos")]
fn platform_open(path: &str) -> Result<(), String> {
    // Applications are real bundles here; the system opener launches the app
    // without revealing a Finder window.
    crate::core::terminals::open_path(path)
}

#[cfg(target_os = "linux")]
fn platform_list() -> Result<Vec<Entry>, String> {
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
fn desktop_entry(path: &Path) -> Option<(String, bool, bool)> {
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
fn platform_open(encoded: &str) -> Result<(), String> {
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

#[cfg(target_os = "windows")]
#[derive(serde::Deserialize)]
struct WindowsApp {
    name: String,
    #[serde(default)]
    aumid: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    icon: String,
}

#[cfg(target_os = "windows")]
fn resolve_windows_path(value: &str) -> String {
    fn join(base: Option<String>, rest: &str) -> Option<String> {
        let base = base?;
        Some(format!(
            "{}\\{}",
            base.trim_end_matches(['\\', '/']),
            rest.trim_start_matches(['\\', '/'])
        ))
    }
    let known = [
        ("{6D809377-6AF0-444B-8957-A3773F02200E}", "ProgramFiles"),
        (
            "{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}",
            "ProgramFiles(x86)",
        ),
        ("{F38BF404-1D43-42F2-9305-67DE0B28FC23}", "windir"),
        ("{D65231B0-B2F1-4857-A4CE-A8E7C6EA7D27}", "SysWOW64"),
    ];
    for (guid, variable) in known {
        if let Some(rest) = value.strip_prefix(guid) {
            let base = if variable == "SysWOW64" {
                std::env::var("windir")
                    .ok()
                    .map(|windir| format!("{}\\SysWOW64", windir.trim_end_matches('\\')))
            } else {
                std::env::var(variable).ok()
            };
            if let Some(path) = join(base, rest) {
                return path;
            }
        }
    }
    value.to_string()
}

#[cfg(target_os = "windows")]
fn looks_like_path(value: &str) -> bool {
    value.len() > 2
        && (value.as_bytes()[1] == b':' || value.starts_with("\\\\") || value.starts_with('{'))
}

#[cfg(target_os = "windows")]
fn platform_list() -> Result<Vec<Entry>, String> {
    let script = r#"$ErrorActionPreference='SilentlyContinue'
$packages=@{}
Get-AppxPackage | ForEach-Object { $packages[$_.PackageFamilyName]=$_.InstallLocation }
$folder=(New-Object -ComObject Shell.Application).Namespace('shell:AppsFolder')
$items=@()
foreach($item in $folder.Items()){
  $aumid=[string]$item.ExtendedProperty('System.AppUserModel.ID')
  $path=[string]$item.Path
  $icon=''
  if($aumid -and $aumid -notmatch '[:\\]' -and $aumid -match '!'){
    $pfn=$aumid.Split('!')[0]
    $location=$packages[$pfn]
    if($location){
      $manifest=Join-Path $location 'AppxManifest.xml'
      if(Test-Path $manifest){
        [xml]$xml=Get-Content $manifest
        $application=@($xml.Package.Applications.Application)[0]
        if($application){
          $logo=$application.VisualElements.Square44x44Logo
          if(-not $logo){$logo=$application.VisualElements.Square150x150Logo}
          if(-not $logo){$logo=$application.VisualElements.Logo}
          if($logo){
            $candidate=Join-Path $location $logo
            if(Test-Path $candidate){$icon=$candidate}
          }
        }
      }
    }
  }
  $items += [pscustomobject]@{ name=$item.Name; aumid=$aumid; path=$path; icon=$icon }
}
ConvertTo-Json -Compress -Depth 2 -InputObject @($items)"#;
    let output = super::windows_shell::run(script)?;
    let value: serde_json::Value =
        serde_json::from_str(&output).map_err(|error| error.to_string())?;
    let values = match value {
        serde_json::Value::Array(values) => values,
        serde_json::Value::Null => vec![],
        other => vec![other],
    };
    let mut entries = Vec::new();
    for value in values {
        let Ok(app) = serde_json::from_value::<WindowsApp>(value) else {
            continue;
        };
        let aumid_path = resolve_windows_path(&app.aumid);
        let shell_path = resolve_windows_path(&app.path);
        let id = if looks_like_path(&app.aumid) {
            format!("path:{aumid_path}")
        } else if !app.aumid.is_empty() {
            format!("aumid:{}", app.aumid)
        } else if !app.path.is_empty() {
            format!("path:{shell_path}")
        } else {
            continue;
        };
        if app.name.trim().is_empty() {
            continue;
        }
        let mut item = entry(app.name, format!("{URI}{}", encode(&id)), "app", 0, false);
        if !app.icon.is_empty() {
            item.link_target = Some(app.icon);
        } else if looks_like_path(&app.path) {
            item.link_target = Some(shell_path);
        } else if !app.path.is_empty() {
            item.link_target = Some(app.path.clone());
        }
        entries.push(item);
    }
    entries.sort_by_key(|left| left.name.to_lowercase());
    Ok(entries)
}

#[cfg(target_os = "windows")]
fn platform_open(encoded: &str) -> Result<(), String> {
    let id = decode(encoded)?;
    if let Some(aumid) = id.strip_prefix("aumid:") {
        let script = format!(
            "$ErrorActionPreference='Stop'; Start-Process ('shell:AppsFolder\\' + {})",
            ps_quote(aumid)
        );
        super::windows_shell::run(&script)?;
        return Ok(());
    }
    if let Some(path) = id.strip_prefix("path:") {
        let status = std::process::Command::new("cmd")
            .args(["/C", "start", "", path])
            .status()
            .map_err(|error| error.to_string())?;
        return if status.success() {
            Ok(())
        } else {
            Err(crate::i18n::t("无法启动应用").into())
        };
    }
    Err(crate::i18n::t("无法启动应用").into())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_list() -> Result<Vec<Entry>, String> {
    Ok(Vec::new())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_open(_encoded: &str) -> Result<(), String> {
    Err(crate::i18n::t("无法启动应用").into())
}

/// Render a real application bundle or shortcut icon into a PNG the asset
/// protocol can show. Core decorations use this only for system-owned apps.
pub fn cached_icon(source: &str) -> Option<String> {
    let path = Path::new(source);
    if !path.exists() {
        return None;
    }
    let cache = dirs::home_dir()?.join(".fiex").join("app-icons");
    std::fs::create_dir_all(&cache).ok()?;
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(source.as_bytes());
    let target = cache.join(format!("{}.png", hex::encode(&digest[..12])));
    if target.exists() {
        return Some(target.to_string_lossy().into_owned());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "ico") {
        if let Ok(image) = image::open(path) {
            image.thumbnail(128, 128).save(&target).ok()?;
            return Some(target.to_string_lossy().into_owned());
        }
    }
    let icon = file_icon_provider::get_file_icon(source, 128).ok()?;
    let image = image::RgbaImage::from_raw(icon.width, icon.height, icon.pixels)?;
    image.save(&target).ok()?;
    Some(target.to_string_lossy().into_owned())
}

fn encode(value: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value)
}

fn decode(value: &str) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn desktop_entries_require_an_application_with_a_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("editor.desktop");
        std::fs::write(
            &file,
            "[Desktop Entry]\nType=Application\nName=Editor\nNoDisplay=false\n",
        )
        .unwrap();
        assert_eq!(desktop_entry(&file), Some(("Editor".into(), false, true)));
        std::fs::write(
            &file,
            "[Desktop Entry]\nType=Application\nName=Hidden\nNoDisplay=true\n",
        )
        .unwrap();
        assert_eq!(desktop_entry(&file), Some(("Hidden".into(), true, true)));
        std::fs::write(&file, "[Desktop Entry]\nType=Link\nName=Link\n").unwrap();
        assert_eq!(desktop_entry(&file), Some(("Link".into(), false, false)));
    }
}
