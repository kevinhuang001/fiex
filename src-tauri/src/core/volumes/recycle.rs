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
        readonly: false,
        link_target: (!original.is_empty()).then(|| original.to_string()),
        allowed_ops: vec![],
        tags: vec![],
        has_cache: false,
        pinned: false,
        uploaded: false,
        sync: (!deleted.is_empty()).then(|| deleted.to_string()),
    }
}

#[cfg(target_os = "windows")]
#[derive(serde::Deserialize)]
struct WindowsRecycleItem {
    index: u64,
    name: String,
    #[serde(default)]
    original: String,
    #[serde(default)]
    deleted: String,
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
  $items += [pscustomobject]@{ index=$index; name=$item.Name; original=[string]$item.ExtendedProperty('System.Recycle.DeletedFrom'); deleted=[string]$item.ExtendedProperty('System.Recycle.DateDeleted'); folder=[bool]$item.IsFolder }
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
        let encoded = encode(&format!("{}|{}", item.index, item.name));
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

#[cfg(target_os = "windows")]
fn verb_script(encoded: &str, verbs: &[&str]) -> Result<(), String> {
    let id = decode(encoded)?;
    let (index, _) = id
        .split_once('|')
        .ok_or_else(|| crate::i18n::t("回收站项目无效").to_string())?;
    let index = index
        .parse::<u64>()
        .map_err(|_| crate::i18n::t("回收站项目无效").to_string())?;
    let names: Vec<String> = verbs.iter().map(|value| ps_quote(value)).collect();
    let script = format!(
        r#"$ErrorActionPreference='Stop'
$bin=(New-Object -ComObject Shell.Application).Namespace(10)
$target=$null
$index=0
foreach($item in $bin.Items()){{ if($index -eq {index}){{ $target=$item; break }}; $index++ }}
if($null -eq $target){{ throw 'recycle item not found' }}
$names=@({names})
$verb=$target.Verbs() | Where-Object {{ $name=($_.Name -replace '&',''); ($names | Where-Object {{ $name -match $_ }}) }} | Select-Object -First 1
if($null -eq $verb){{ throw 'recycle action unavailable' }}
$verb.DoIt()"#,
        names = names.join(",")
    );
    super::windows_shell::run(&script).map(|_| ())
}

#[cfg(target_os = "windows")]
pub fn restore(encoded: &str) -> Result<(), String> {
    verb_script(encoded, &["还原", "Restore", "Put Back", "放回原处"])
}

#[cfg(target_os = "windows")]
pub fn purge(encoded: &str) -> Result<(), String> {
    verb_script(encoded, &["删除", "Delete", "Remove"])
}

#[cfg(target_os = "windows")]
pub fn empty() -> Result<usize, String> {
    let count = list().map(|items| items.len()).unwrap_or(0);
    if count == 0 {
        return Ok(0);
    }
    // Clear-RecycleBin fails on some Windows installations even with a valid
    // bin; the shell's own "Empty Recycle Bin" verb is the reliable fallback.
    // It belongs to the bin root, so Explorer is never opened.
    super::windows_shell::run(
        r#"$ErrorActionPreference='Stop'
try {
  Clear-RecycleBin -Force -Confirm:$false
  Write-Output 'cleared'
} catch {
  $bin=(New-Object -ComObject Shell.Application).Namespace(10)
  $verb=$bin.Self.Verbs() | Where-Object { ($_.Name -replace '&','') -match 'Empty Recycle Bin|清空回收站' } | Select-Object -First 1
  if(-not $verb){ throw 'recycle bin has no empty verb' }
  $verb.DoIt()
  Write-Output 'verb'
}"#,
    )?;
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
