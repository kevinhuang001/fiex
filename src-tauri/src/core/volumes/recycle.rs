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
#[derive(serde::Serialize, serde::Deserialize)]
struct WindowsRecycleItem {
    index: u64,
    name: String,
    #[serde(default)]
    original: String,
    #[serde(default)]
    deleted: String,
    #[serde(default, rename = "deletedUnix")]
    deleted_unix: i64,
    #[serde(default)]
    folder: bool,
}

#[cfg(target_os = "windows")]
pub fn list() -> Result<Vec<Entry>, String> {
    let script = r#"$ErrorActionPreference='SilentlyContinue'
$bin=(New-Object -ComObject Shell.Application).Namespace(10)
$items=@()
$index=0
foreach($item in $bin.Items()){
  $deleted=$item.ExtendedProperty('System.Recycle.DateDeleted')
  $deletedUnix=0
  try { if($deleted) { $deletedUnix=([DateTimeOffset]([datetime]$deleted)).ToUnixTimeSeconds() } } catch { }
  $items += [pscustomobject]@{ index=$index; name=$item.Name; original=[string]$item.ExtendedProperty('System.Recycle.DeletedFrom'); deleted=[string]$deleted; deletedUnix=[long]$deletedUnix; folder=[bool]$item.IsFolder }
  $index++
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
        let Ok(item) = serde_json::from_value::<WindowsRecycleItem>(value) else {
            continue;
        };
        if item.name.trim().is_empty() {
            continue;
        }
        // The index is only a shell-enumeration position, not a stable item
        // identity. Capture the metadata too and reject a reordered/stale
        // selection when executing destructive verbs.
        let encoded = encode(&serde_json::to_string(&item).map_err(|e| e.to_string())?);
        entries.push(entry(
            item.name,
            format!("{URI}{encoded}"),
            item.folder,
            &item.original,
            &item.deleted,
        ));
    }
    Ok(entries)
}

/// Reconstruct the metadata captured in an opaque Recycle Bin identity without
/// enumerating the Shell namespace. The identity is only a menu/list snapshot;
/// destructive operations still enumerate and validate the current item before
/// invoking its Shell verb.
#[cfg(target_os = "windows")]
pub fn stat(uri: &str) -> Result<Entry, String> {
    let encoded = uri
        .strip_prefix(URI)
        .filter(|encoded| !encoded.is_empty() && !encoded.contains('/'))
        .ok_or("InvalidVfsUri: invalid Recycle Bin item")?;
    let item: WindowsRecycleItem = serde_json::from_str(&decode(encoded)?)
        .map_err(|_| crate::i18n::t("回收站项目无效").to_string())?;
    Ok(entry(
        item.name,
        uri.to_owned(),
        item.folder,
        &item.original,
        &item.deleted,
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn stat(_uri: &str) -> Result<Entry, String> {
    Err("Unsupported: Recycle Bin item is unavailable".into())
}

#[cfg(target_os = "windows")]
fn verb_script(encoded: &str, verbs: &[&str]) -> Result<(), String> {
    let item: WindowsRecycleItem = serde_json::from_str(&decode(encoded)?)
        .map_err(|_| crate::i18n::t("回收站项目无效").to_string())?;
    let names: Vec<String> = verbs.iter().map(|value| ps_quote(value)).collect();
    let index = item.index;
    let expected_name = ps_quote(&item.name);
    let expected_origin = ps_quote(&item.original);
    let expected_deleted = ps_quote(&item.deleted);
    let expected_folder = if item.folder { "$true" } else { "$false" };
    let script = format!(
        r#"$ErrorActionPreference='Stop'
$bin=(New-Object -ComObject Shell.Application).Namespace(10)
$target=$null
$index=0
foreach($item in $bin.Items()){{
  if($index -eq {index}){{
    if($item.Name -cne {expected_name} -or
       [string]$item.ExtendedProperty('System.Recycle.DeletedFrom') -cne {expected_origin} -or
       [string]$item.ExtendedProperty('System.Recycle.DateDeleted') -cne {expected_deleted} -or
       [bool]$item.IsFolder -ne {expected_folder}){{
      throw 'StaleRef: recycle item changed since enumeration'
    }}
    $target=$item
    break
  }}
  $index++
}}
if($null -eq $target){{ throw 'recycle item not found' }}
$names=@({names})
$verb=$target.Verbs() | Where-Object {{ $name=($_.Name -replace '&',''); ($names | Where-Object {{ $name -match $_ }}) }} | Select-Object -First 1
if($null -eq $verb){{ throw 'recycle action unavailable' }}
$verb.DoIt()"#,
        names = names.join(","),
        expected_name = expected_name,
        expected_origin = expected_origin,
        expected_deleted = expected_deleted,
        expected_folder = expected_folder,
    );
    super::windows_shell::run(&script).map(|_| ())
}

#[cfg(target_os = "windows")]
pub fn restore(encoded: &str) -> Result<(), String> {
    verb_script(encoded, &["还原", "Restore", "Put Back", "放回原处"])
}

#[cfg(target_os = "windows")]
pub fn purge(encoded: &str) -> Result<(), String> {
    let expected: WindowsRecycleItem = serde_json::from_str(&decode(encoded)?)
        .map_err(|_| crate::i18n::t("回收站项目无效").to_string())?;
    let current = list()?;
    let uri = format!("{URI}{encoded}");
    if !current.iter().any(|entry| entry.path == uri) {
        return Err("StaleRef: recycle item changed since enumeration".into());
    }

    let items = trash::os_limited::list().map_err(|error| error.to_string())?;
    let mut matches = items.into_iter().filter(|item| {
        item.name.to_string_lossy() == expected.name
            && (expected.original.is_empty()
                || same_windows_path(&item.original_parent, &expected.original))
            && (expected.deleted_unix == 0 || item.time_deleted == expected.deleted_unix)
    });
    let selected = matches
        .next()
        .ok_or_else(|| "StaleRef: recycle item changed since enumeration".to_owned())?;
    if matches.next().is_some() {
        return Err("StaleRef: recycle item identity is ambiguous; refresh the list".into());
    }

    trash::os_limited::purge_all([selected]).map_err(|error| error.to_string())?;
    if trash::os_limited::list()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|item| recycle_item_matches(item, &expected))
    {
        return Err("Windows did not permanently delete the selected Recycle Bin item".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn recycle_item_matches(item: &trash::TrashItem, expected: &WindowsRecycleItem) -> bool {
    item.name.to_string_lossy() == expected.name
        && (expected.original.is_empty()
            || same_windows_path(&item.original_parent, &expected.original))
        && (expected.deleted_unix == 0 || item.time_deleted == expected.deleted_unix)
}

#[cfg(target_os = "windows")]
fn same_windows_path(left: &std::path::Path, right: &str) -> bool {
    fn normalized(value: &str) -> String {
        let value = value.replace('/', "\\");
        let value = if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else if let Some(ordinary) = value.strip_prefix(r"\\?\") {
            ordinary.to_owned()
        } else {
            value
        };
        let mut value = value.to_ascii_lowercase();
        while value.ends_with('\\') && value.len() > 3 {
            value.pop();
        }
        value
    }
    normalized(&left.to_string_lossy()) == normalized(right)
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

#[cfg(target_os = "windows")]
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
