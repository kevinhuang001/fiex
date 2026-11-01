//! The "Applications" virtual folder. Core enumerates installed applications
//! itself so opening the entry never has to launch Finder or Explorer.

use super::application_folders;
mod platform;
use crate::core::files::Entry;
use crate::core::fs::directory_cache;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

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

fn user_encoded(uri: &str) -> Option<&str> {
    uri.strip_prefix(USER_PREFIX)
}

pub fn is_user(path: &str) -> bool {
    user_encoded(path).is_some()
}

fn user_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".fiex")
        .join("applications")
}

/// The writable side of Applications is a sandboxed local store. URI payloads
/// are relative to that store, including nested user-created directories.
pub fn stored_path(uri: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(user_dir()).map_err(|e| e.to_string())?;
    let root = dunce::canonicalize(user_dir()).map_err(|e| e.to_string())?;
    if uri == URI {
        return Ok(root);
    }
    let relative = decode(user_encoded(uri).ok_or("not a stored application")?)?;
    if relative.is_empty()
        || relative.contains(['\\', ':', '\0'])
        || Path::new(&relative)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("invalid application store path".into());
    }
    let target = root.join(relative);
    let canonical = dunce::canonicalize(&target).map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root) || canonical == root {
        return Err("application store escape".into());
    }
    Ok(canonical)
}

/// Validate an Applications user URI without requiring the final node to
/// exist. A newly created child is confined to the canonical private root;
/// every existing ancestor is canonicalized to reject symlink escapes.
pub fn candidate_path(uri: &str) -> Result<PathBuf, String> {
    let root = stored_path(URI)?;
    let encoded = user_encoded(uri).ok_or("not a user application")?;
    let relative = decode(encoded)?;
    if relative.is_empty()
        || relative.contains(['\\', ':', '\0'])
        || Path::new(&relative)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("invalid application store path".into());
    }
    let target = root.join(relative);
    let mut ancestor = target.as_path();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(
            ancestor
                .file_name()
                .ok_or("invalid application child path")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().ok_or("missing application ancestor")?;
    }
    let mut resolved = dunce::canonicalize(ancestor).map_err(|e| e.to_string())?;
    if !resolved.starts_with(&root) {
        return Err("application store escape".into());
    }
    for child in missing.iter().rev() {
        resolved.push(child);
    }
    if resolved == root {
        return Err("invalid application child path".into());
    }
    Ok(resolved)
}

/// Ensure a generated child belongs directly to the selected Applications
/// directory. This is used when Core binds a provider-returned ResourceRef.
pub fn validate_child(parent: &str, child: &str) -> Result<(), String> {
    let parent_path = if parent == URI {
        stored_path(parent)?
    } else if is_user(parent) {
        candidate_path(parent)?
    } else {
        return Err("ScopeDenied: invalid application parent".into());
    };
    let child_path = candidate_path(child)?;
    if child_path.parent() != Some(parent_path.as_path()) {
        return Err("ScopeDenied: child outside selected application directory".into());
    }
    Ok(())
}

pub fn stored_uri(path: &Path) -> Result<String, String> {
    let root = dunce::canonicalize(user_dir()).map_err(|e| e.to_string())?;
    let rel = path
        .strip_prefix(root)
        .map_err(|_| "not in application store")?;
    Ok(format!(
        "{USER_PREFIX}{}",
        encode(&rel.to_string_lossy().replace('\\', "/"))
    ))
}

pub fn stored_entry(uri: &str) -> Result<Entry, String> {
    let path = stored_path(uri)?;
    let mut item = crate::core::files::entry(&path)?;
    item.path = uri.into();
    // Internal backing paths must not escape through metadata supplied to
    // extension providers. Launch and IO resolve the store URI inside Core.
    item.link_target = None;
    // Preserve the real content MIME for user-owned files: Preview, Archive,
    // Checksum and other Features must match exactly as on local storage.
    // Launch support is a separate application action, never a MIME override.
    item.allowed_ops = if item.is_dir {
        vec![
            "stat".into(),
            "list".into(),
            "search".into(),
            "mkdir".into(),
            "rename".into(),
            "remove".into(),
        ]
    } else {
        vec![
            "stat".into(),
            "read".into(),
            "write".into(),
            "rename".into(),
            "remove".into(),
        ]
    };
    Ok(item)
}

pub fn list_at(uri: &str) -> Result<Vec<Entry>, String> {
    if uri == URI {
        return list();
    }
    let entries = list_stored(&stored_path(uri)?)?;
    let mapped = application_folders::apps_in_folder(uri);
    if mapped.is_empty() {
        return Ok(entries);
    }
    Ok(folder_listing(entries, &system_entries()?, &mapped))
}

fn folder_listing(mut entries: Vec<Entry>, system: &[Entry], mapped: &[String]) -> Vec<Entry> {
    for app in mapped {
        let Some(entry) = system.iter().find(|entry| entry.path == *app).cloned() else {
            continue;
        };
        if !entries.iter().any(|candidate| candidate.path == entry.path) {
            entries.push(entry);
        }
    }
    entries
}

fn list_stored(dir: &Path) -> Result<Vec<Entry>, String> {
    let mut result = Vec::new();
    for item in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = item.map_err(|e| e.to_string())?.path();
        let uri = stored_uri(&path)?;
        // Escaping symlinks are omitted instead of granting host content.
        if let Ok(item) = stored_entry(&uri) {
            result.push(item);
        }
    }
    result.sort_by_key(|item| item.name.to_lowercase());
    Ok(result)
}
fn list_user() -> Vec<Entry> {
    stored_path(URI)
        .ok()
        .and_then(|root| list_stored(&root).ok())
        .unwrap_or_default()
}

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
        application_folders::rename_folder(old_uri, new_uri)?;
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
    if target.is_dir() {
        // Deleting an application folder is non-destructive for applications:
        // real children return to the root and mapped system apps follow.
        let mut moved = Vec::new();
        let children = std::fs::read_dir(&target)
            .map_err(|error| error.to_string())?
            .map(|item| item.map(|item| item.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for child in children {
            let name = child.file_name().ok_or("application child has no name")?;
            let destination = unique_child(&root, name);
            std::fs::rename(&child, &destination).map_err(|error| error.to_string())?;
            if let Ok(entry) = stored_entry(&stored_uri(&destination)?) {
                moved.push(entry);
            }
        }
        let returned = application_folders::forget_folder(uri)?;
        std::fs::remove_dir(&target).map_err(|error| error.to_string())?;
        directory_cache::drop_subtree(uri);
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        for entry in moved {
            directory_cache::add_entry(URI, entry);
        }
        for app in returned {
            if let Ok(entry) = system_entry(&app) {
                directory_cache::add_entry(URI, entry);
            }
        }
        Ok(())
    } else {
        std::fs::remove_file(target).map_err(|error| error.to_string())?;
        directory_cache::remove_entry(&parent_uri, uri);
        directory_cache::remove_entry(URI, uri);
        Ok(())
    }
}

fn unique_child(root: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let mut target = root.join(name);
    if !target.exists() {
        return target;
    }
    let name = name.to_string_lossy();
    let path = Path::new(name.as_ref());
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("application");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let mut index = 2;
    loop {
        target = root.join(format!("{stem} ({index}){extension}"));
        if !target.exists() {
            return target;
        }
        index += 1;
    }
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

#[cfg(all(test, target_os = "windows"))]
mod windows_target_tests {
    use super::resolve_windows_executable;

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
