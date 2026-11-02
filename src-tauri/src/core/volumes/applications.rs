//! The "Applications" virtual folder. Core enumerates installed applications
//! itself so opening the entry never has to launch Finder or Explorer.

use super::application_folders;
mod platform;
mod store;
use crate::core::files::Entry;
use crate::core::fs::directory_cache;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
pub(super) use store::list_user;
pub use store::{
    after_rename, candidate_path, create_stored, is_user, list_at, remove_user, rename_stored,
    stored_entry, stored_path, stored_uri, validate_child,
};
#[cfg(test)]
pub(super) use store::{folder_listing, user_encoded};

pub const URI: &str = "applications://";
/// User-created application-store nodes live directly under the view root.
/// The `~` marker keeps them distinct from system application identities,
/// which are base64-encoded host paths.
pub const USER_PREFIX: &str = "applications://~";

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
        allowed_ops: if readonly {
            vec!["stat".into()]
        } else {
            vec!["stat".into(), "remove".into()]
        },
        tags: vec![],
        has_cache: false,
        pinned: false,
        uploaded: false,
        sync: None,
    }
}

pub fn list() -> Result<Vec<Entry>, String> {
    let system = refresh_system_entries()?;
    let mapped = application_folders::mapped_apps();
    Ok(root_listing(system, list_user(), &mapped))
}

fn root_listing(system: Vec<Entry>, user: Vec<Entry>, mapped: &HashSet<String>) -> Vec<Entry> {
    let mut entries: Vec<Entry> = system
        .into_iter()
        .filter(|entry| !mapped.contains(&entry.path))
        .collect();
    entries.extend(user);
    entries
}

/// System applications live in the platform's installed-app namespace, not in
/// a real directory. Keep their last enumeration so a folder listing does not
/// re-run the platform query after a move.
fn system_cache() -> &'static Mutex<Option<Vec<Entry>>> {
    static CACHE: OnceLock<Mutex<Option<Vec<Entry>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn cached_system_entries() -> Option<Vec<Entry>> {
    system_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

fn store_system_entries(entries: &[Entry]) {
    *system_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(entries.to_vec());
}

fn refresh_system_entries() -> Result<Vec<Entry>, String> {
    let entries = platform::platform_list()?;
    store_system_entries(&entries);
    let ids = entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    application_folders::refresh(&ids);
    Ok(entries)
}

/// All installed applications, including those currently mapped into folders.
pub fn system_entries() -> Result<Vec<Entry>, String> {
    if let Some(entries) = cached_system_entries() {
        return Ok(entries);
    }
    let entries = platform::platform_list()?;
    store_system_entries(&entries);
    Ok(entries)
}

pub fn is_system_app(path: &str) -> bool {
    path != URI && path.starts_with(URI) && !is_user(path)
}

pub fn is_mapped(path: &str) -> bool {
    application_folders::is_mapped(path)
}

pub fn folder_for(path: &str) -> Option<String> {
    application_folders::folder_for(path)
}

pub fn system_entry(path: &str) -> Result<Entry, String> {
    if !is_system_app(path) {
        return Err("not a system application".into());
    }
    system_entries()?
        .into_iter()
        .find(|entry| entry.path == path)
        .ok_or_else(|| crate::i18n::t("应用不存在").to_string())
}

/// Move system applications into a folder (or back to the root). User-store
/// files use the regular filesystem transfer instead.
pub fn move_system_apps(paths: &[String], destination: &str) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut entries = Vec::with_capacity(paths.len());
    let mut previous = Vec::with_capacity(paths.len());
    for path in paths {
        if !is_system_app(path) {
            return Err("not a system application".into());
        }
        entries.push(system_entry(path)?);
        previous.push(application_folders::folder_for(path));
    }
    application_folders::move_apps(paths, destination)?;
    for ((app, entry), previous) in paths.iter().zip(entries).zip(previous) {
        directory_cache::remove_entry(URI, app);
        if let Some(folder) = previous {
            directory_cache::remove_entry(&folder, app);
        }
        if destination == URI {
            directory_cache::add_entry(URI, entry);
        } else {
            directory_cache::add_entry(destination, entry);
        }
    }
    Ok(())
}

/// Context-menu delete for an application entry. Mapped system apps return to
/// the root; user-store files are removed through the normal store rules.
pub fn remove_entries(paths: &[String]) -> Result<(), String> {
    // Review the whole selection before any mutation; mixed selections cannot
    // accidentally remove user data before hitting a protected system app.
    if paths.iter().any(|path| !is_user(path)) {
        return Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into());
    }
    for path in paths {
        remove_user(path)?;
    }
    Ok(())
}

/// Move user-store entries to the platform Trash. Mapped system apps in a
/// trashed folder return to the root.
pub fn trash_entries(paths: &[String]) -> Result<(), String> {
    // Review the whole selection before any mutation; mixed selections cannot
    // accidentally remove user data before hitting a protected system app.
    if paths.iter().any(|path| !is_user(path)) {
        return Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into());
    }
    for path in paths {
        trash_user(path)?;
    }
    Ok(())
}

fn trash_user(uri: &str) -> Result<(), String> {
    let target = stored_path(uri)?;
    let parent = target.parent().ok_or("application item has no parent")?;
    let root = stored_path(URI)?;
    let parent_uri = if parent == root {
        URI.to_string()
    } else {
        stored_uri(parent)?
    };
    let directory = target.is_dir();
    let path = target.to_string_lossy().into_owned();
    let result = crate::core::operations::recycle(std::slice::from_ref(&path))?;
    if let Some(error) = result.errors.into_iter().next() {
        return Err(error);
    }
    directory_cache::remove_entry(&parent_uri, uri);
    directory_cache::remove_entry(URI, uri);
    if directory {
        let returned = application_folders::forget_folder(uri)?;
        directory_cache::drop_subtree(uri);
        for app in returned {
            if let Ok(entry) = system_entry(&app) {
                directory_cache::add_entry(URI, entry);
            }
        }
    }
    Ok(())
}

pub fn open(encoded: &str) -> Result<(), String> {
    let uri = format!("{URI}{encoded}");
    if is_user(&uri) {
        return platform::open_stored(&stored_path(&uri)?);
    }
    platform::platform_open(encoded)
}

/// Executable file types that the Applications view is allowed to launch.
/// Documents and website shortcuts share the Start Menu namespace, so the
/// enumeration has to reject them explicitly.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn is_executable_target(value: &str) -> bool {
    if value.contains("://") {
        return false;
    }
    let extension = Path::new(value)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "exe" | "com" | "bat" | "cmd" | "msc" | "cpl" | "scr" | "ps1" | "appref-ms" | "jar"
    )
}

/// Render a real application bundle or shortcut icon into a PNG the asset
/// protocol can show. Core decorations use this only for system-owned apps.
#[cfg(feature = "desktop")]
pub fn cached_icon(source: &str) -> Option<String> {
    let path = Path::new(source);
    if !source.starts_with("shell:AppsFolder\\") && !path.exists() {
        return None;
    }
    let cache = dirs::home_dir()?.join(".fiex").join("app-icons");
    std::fs::create_dir_all(&cache).ok()?;
    use sha2::{Digest, Sha256};
    let stamp = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|t| t.as_secs())
        .unwrap_or(0);
    let digest = Sha256::digest(format!("v3:{source}:{stamp}").as_bytes());
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
    #[cfg(target_os = "windows")]
    if super::windows_shell::extract_icon(source, &target).is_ok() {
        return Some(target.to_string_lossy().into_owned());
    }
    let icon = file_icon_provider::get_file_icon(source, 128).ok()?;
    let image = image::RgbaImage::from_raw(icon.width, icon.height, icon.pixels)?;
    image.save(&target).ok()?;
    Some(target.to_string_lossy().into_owned())
}

#[cfg(not(feature = "desktop"))]
pub fn cached_icon(_source: &str) -> Option<String> {
    None
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
        assert_eq!(
            platform::desktop_entry(&file),
            Some(("Editor".into(), false, true))
        );
        std::fs::write(
            &file,
            "[Desktop Entry]\nType=Application\nName=Hidden\nNoDisplay=true\n",
        )
        .unwrap();
        assert_eq!(
            platform::desktop_entry(&file),
            Some(("Hidden".into(), true, true))
        );
        std::fs::write(&file, "[Desktop Entry]\nType=Link\nName=Link\n").unwrap();
        assert_eq!(
            platform::desktop_entry(&file),
            Some(("Link".into(), false, false))
        );
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_target_tests {
    use super::platform::resolve_windows_executable;

    #[test]
    fn valid_system_known_folder_target_is_launchable() {
        let target = resolve_windows_executable(r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\cmd.exe")
            .expect("Windows System32 cmd.exe should exist");
        assert!(target.to_lowercase().ends_with(r"\system32\cmd.exe"));
    }

    #[test]
    fn unresolved_and_missing_targets_are_not_listed() {
        assert!(
            resolve_windows_executable(r"{00000000-0000-0000-0000-000000000000}\missing.exe")
                .is_none()
        );
        assert!(resolve_windows_executable(
            r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\fiex-nonexistent-test.exe"
        )
        .is_none());
        assert!(resolve_windows_executable(r"C:\fiex-nonexistent-test.exe").is_none());
        assert!(resolve_windows_executable(r"C:\Windows\readme.txt").is_none());
    }
}

#[cfg(test)]
mod executable_target_tests {
    use super::is_executable_target;

    #[test]
    fn executables_are_kept_including_uninstallers() {
        for path in [
            r"C:\Program Files\App\app.exe",
            r"C:\Program Files\App\unins000.exe",
            r"C:\Program Files\App\Uninstall App.exe",
            r"C:\Program Files\App\tool.cmd",
            r"C:\Windows\System32\app.msc",
            r"C:\Users\me\AppData\Roaming\Microsoft\Installer\{ABC}\app.appref-ms",
        ] {
            assert!(is_executable_target(path), "{path} should be launched");
        }
    }

    #[test]
    fn documents_and_websites_are_filtered() {
        for path in [
            r"C:\Program Files\App\readme.pdf",
            r"C:\Program Files\App\manual.docx",
            r"C:\Program Files\App\release-notes.txt",
            "https://example.com",
            "https://example.com/download",
            r"C:\Program Files\App\website.url",
            "",
        ] {
            assert!(!is_executable_target(path), "{path} should be filtered");
        }
    }
}

#[cfg(test)]
mod folder_listing_tests {
    use super::*;

    #[test]
    fn system_application_metadata_is_read_only_even_before_capability_matching() {
        let protected = entry(
            "System App".into(),
            "applications://c3lzdGVt".into(),
            "app",
            0,
            true,
        );
        assert!(protected.readonly);
        assert_eq!(protected.allowed_ops, vec!["stat"]);
        let user = entry(
            "Shortcut".into(),
            "applications://~dXNlcg".into(),
            "app",
            0,
            false,
        );
        assert!(!user.readonly);
        assert!(user.allowed_ops.iter().any(|op| op == "remove"));
    }

    #[test]
    fn user_uris_use_the_canonical_store_prefix() {
        assert_eq!(user_encoded("applications://~abc"), Some("abc"));
        assert!(is_user("applications://~abc"));
        assert!(!is_user("applications://user/abc"));
        // System application identities are never user-store nodes.
        assert!(!is_user("applications://YWJj"));
    }

    fn item(name: &str, path: &str, is_dir: bool) -> Entry {
        Entry {
            name: name.into(),
            path: path.into(),
            is_dir,
            ..Default::default()
        }
    }

    #[test]
    fn mapped_system_apps_leave_the_root_and_appear_once_in_the_folder() {
        let app = item("Editor", "applications://YXBw", false);
        let other = item("Browser", "applications://YnJvd3Nlcg", false);
        let folder = item("Work", "applications://~d29yaw", true);
        let mapped = HashSet::from([app.path.clone()]);
        let root = root_listing(
            vec![app.clone(), other.clone()],
            vec![folder.clone()],
            &mapped,
        );
        assert!(root.iter().any(|entry| entry.path == other.path));
        assert!(root.iter().any(|entry| entry.path == folder.path));
        assert!(!root.iter().any(|entry| entry.path == app.path));

        let children = folder_listing(
            vec![folder],
            std::slice::from_ref(&app),
            std::slice::from_ref(&app.path),
        );
        assert_eq!(
            children
                .iter()
                .filter(|entry| entry.path == app.path)
                .count(),
            1
        );
    }

    #[test]
    fn duplicate_real_and_mapped_entries_are_collapsed() {
        let app = item("Editor", "applications://YXBw", false);
        let entries = folder_listing(
            vec![app.clone()],
            std::slice::from_ref(&app),
            std::slice::from_ref(&app.path),
        );
        assert_eq!(entries.len(), 1);
    }
}
