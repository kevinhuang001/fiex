//! Windows Recycle Bin as an in-app virtual folder. Enumeration and item verbs
//! go through the shell namespace without opening an Explorer window.

use crate::core::files::Entry;

pub const URI: &str = "recyclebin://";

pub fn is_virtual(path: &str) -> bool {
    path == URI || path.starts_with(URI)
}

#[cfg(target_os = "windows")]
fn entry(name: String, path: String, is_dir: bool, original: &str, deleted: &str) -> Entry {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_lowercase())
        .unwrap_or_default();
    Entry {
        path,
        name,
        is_dir,
        is_symlink: false,
        size: 0,
        modified: 0,
        created: 0,
        extension,
        mime: String::new(),
        link_target: (!original.is_empty()).then(|| original.to_string()),
        // Recycle Bin items expose metadata and generic Remove; Restore stays
        // a dedicated action because it has no filesystem mutation equivalent.
        allowed_ops: vec!["stat".into(), "remove".into()],
        tags: vec![],
        has_cache: false,
        pinned: false,
        storage_projection: Default::default(),
        uploaded: false,
        sync: (!deleted.is_empty()).then(|| deleted.to_string()),
    }
}

#[cfg(target_os = "windows")]
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct WindowsRecycleItem {
    /// Stable Shell parsing identity supplied by the same API used for every
    /// subsequent stat, restore and purge operation.
    id: String,
    name: String,
    original: String,
    deleted_unix: i64,
    folder: bool,
}

#[cfg(target_os = "windows")]
pub fn list() -> Result<Vec<Entry>, String> {
    trash::os_limited::list()
        .map_err(|error| error.to_string())?
        .iter()
        .map(|item| {
            let identity = identity(item)?;
            let encoded = encode(&serde_json::to_string(&identity).map_err(|e| e.to_string())?);
            Ok(entry(
                identity.name.clone(),
                format!("{URI}{encoded}"),
                identity.folder,
                &identity.original,
                &identity.deleted_unix.to_string(),
            ))
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn identity(item: &trash::TrashItem) -> Result<WindowsRecycleItem, String> {
    Ok(WindowsRecycleItem {
        id: item
            .id
            .to_str()
            .ok_or("Recycle Bin item identity is not valid Unicode")?
            .to_owned(),
        name: item
            .name
            .to_str()
            .ok_or("Recycle Bin item name is not valid Unicode")?
            .to_owned(),
        original: item
            .original_parent
            .to_str()
            .ok_or("Recycle Bin source path is not valid Unicode")?
            .to_owned(),
        deleted_unix: item.time_deleted,
        folder: matches!(
            trash::os_limited::metadata(item)
                .map_err(|error| error.to_string())?
                .size,
            trash::TrashItemSize::Entries(_)
        ),
    })
}

#[cfg(target_os = "windows")]
fn resolve_item(encoded: &str) -> Result<(WindowsRecycleItem, trash::TrashItem), String> {
    let expected: WindowsRecycleItem = serde_json::from_str(&decode(encoded)?)
        .map_err(|_| crate::i18n::t("回收站项目无效").to_string())?;
    let mut matches = trash::os_limited::list()
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|item| item.id.to_str() == Some(expected.id.as_str()));
    let selected = matches
        .next()
        .ok_or_else(|| "StaleRef: Recycle Bin item no longer exists".to_owned())?;
    if matches.next().is_some() {
        return Err("StaleRef: Recycle Bin item identity is ambiguous".into());
    }
    if identity(&selected)? != expected {
        return Err("StaleRef: Recycle Bin item changed since enumeration".into());
    }
    Ok((expected, selected))
}

/// Reconstruct the selected item from the same native Shell identity source
/// used by enumeration. Enumeration order and localized display formatting
/// are not part of the identity.
#[cfg(target_os = "windows")]
pub fn stat(uri: &str) -> Result<Entry, String> {
    let encoded = uri
        .strip_prefix(URI)
        .filter(|encoded| !encoded.is_empty() && !encoded.contains('/'))
        .ok_or("InvalidVfsUri: invalid Recycle Bin item")?;
    let (item, _) = resolve_item(encoded)?;
    Ok(entry(
        item.name,
        uri.to_owned(),
        item.folder,
        &item.original,
        &item.deleted_unix.to_string(),
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn stat(_uri: &str) -> Result<Entry, String> {
    Err("Unsupported: Recycle Bin item is unavailable".into())
}

#[cfg(target_os = "windows")]
pub fn restore(encoded: &str) -> Result<(), String> {
    let (_, item) = resolve_item(encoded)?;
    let id = item.id.clone();
    trash::os_limited::restore_all([item]).map_err(|error| error.to_string())?;
    if trash::os_limited::list()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|current| current.id == id)
    {
        return Err("Windows did not restore the selected Recycle Bin item".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn purge(encoded: &str) -> Result<(), String> {
    let (_, item) = resolve_item(encoded)?;
    trash::os_limited::purge_all([&item]).map_err(|error| error.to_string())?;
    if trash::os_limited::list()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|current| current.id == item.id)
    {
        return Err("Windows did not permanently delete the selected Recycle Bin item".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn empty() -> Result<usize, String> {
    let items = trash::os_limited::list().map_err(|error| error.to_string())?;
    let count = items.len();
    if count == 0 {
        return Ok(0);
    }
    trash::os_limited::purge_all(items).map_err(|error| error.to_string())?;
    if !trash::os_limited::is_empty().map_err(|error| error.to_string())? {
        return Err("Windows did not empty the Recycle Bin".into());
    }
    Ok(count)
}

#[cfg(not(target_os = "windows"))]
pub fn list() -> Result<Vec<Entry>, String> {
    Ok(Vec::new())
}

#[cfg(not(target_os = "windows"))]
pub fn restore(_encoded: &str) -> Result<(), String> {
    Err(crate::i18n::t("当前系统请使用系统废纸篓").into())
}

#[cfg(not(target_os = "windows"))]
pub fn purge(_encoded: &str) -> Result<(), String> {
    Err(crate::i18n::t("当前系统请使用系统废纸篓").into())
}

#[cfg(not(target_os = "windows"))]
pub fn empty() -> Result<usize, String> {
    Err(crate::i18n::t("当前系统请使用系统废纸篓").into())
}

#[cfg(target_os = "windows")]
fn encode(value: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value)
}

#[cfg(target_os = "windows")]
fn decode(value: &str) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}
