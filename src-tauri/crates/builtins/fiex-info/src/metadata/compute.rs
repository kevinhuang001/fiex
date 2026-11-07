use super::common::{display_parent, fallback_name, resource_kind, size_text, timestamp_text};
use crate::{InfoMetadataInput, Localization};
use fiex_provider_contracts::{InfoFieldKey, InfoFieldResult, ResourceMetadata};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub fn compute_metadata_fields(
    input: &InfoMetadataInput,
    fields: &[InfoFieldKey],
    localization: &Localization,
) -> Result<Vec<InfoFieldResult>, String> {
    let display = input.display.as_str();
    let local = input.local.as_path();
    let is_local = input.is_local;
    let stat = input.stat.as_ref();
    let projected = input.metadata.as_ref();
    let cancelled = input.cancelled.as_ref();
    fields
        .iter()
        .map(|field| {
            let value = match (field.section_id.as_str(), field.field_id.as_str()) {
                ("basic", "name") => projected
                    .map(|value| value.name.clone())
                    .filter(|value| !value.is_empty())
                    .or_else(|| Some(fallback_name(display))),
                ("basic", "kind") => {
                    Some(localization.text(match resource_kind(stat, projected) {
                        "directory" => "文件夹",
                        "symlink" | "link" => "符号链接",
                        _ => "文件",
                    }))
                }
                ("basic", "location") => Some(display_parent(display)),
                ("basic", "extension") => projected
                    .and_then(|value| value.extension.clone())
                    .or_else(|| {
                        local
                            .extension()
                            .map(|value| value.to_string_lossy().to_ascii_lowercase())
                    }),
                ("basic", "mime") => projected.and_then(|value| value.mime.clone()),
                ("basic", "size") if resource_kind(stat, projected) != "directory" => projected
                    .and_then(|value| value.size)
                    .or_else(|| stat.filter(|value| value.is_file()).map(fs::Metadata::len))
                    .map(size_text),
                ("basic", "linkTarget") if is_local => fs::read_link(local)
                    .ok()
                    .map(|value| value.to_string_lossy().into_owned()),
                ("basic", "targetExists") if is_local => fs::read_link(local).ok().map(|target| {
                    let absolute = if target.is_absolute() {
                        target
                    } else {
                        local
                            .parent()
                            .unwrap_or_else(|| Path::new("."))
                            .join(target)
                    };
                    localization.text(if absolute.exists() { "是" } else { "否" })
                }),
                ("times", id) => timestamp_value(id, stat, projected),
                ("filesystem", "readonly") => stat.map(|value| {
                    localization.text(if value.permissions().readonly() {
                        "是"
                    } else {
                        "否"
                    })
                }),
                ("filesystem", "hidden") if is_local => {
                    Some(localization.text(if is_hidden(local, stat) { "是" } else { "否" }))
                }
                ("filesystem", id) if is_local => filesystem_value(id, stat, localization),
                ("directoryStats", id) if is_local => {
                    Some(directory_stat_value(local, id, cancelled)?)
                }
                _ => None,
            };
            Ok(InfoFieldResult {
                section_id: field.section_id.clone(),
                field_id: field.field_id.clone(),
                unavailable: value.is_none(),
                value,
            })
        })
        .collect()
}

fn timestamp_value(
    id: &str,
    stat: Option<&fs::Metadata>,
    projected: Option<&ResourceMetadata>,
) -> Option<String> {
    let seconds = match id {
        "created" => stat
            .and_then(|metadata| metadata.created().ok())
            .and_then(unix_seconds),
        "modified" => projected
            .and_then(|metadata| metadata.modified)
            .or_else(|| {
                stat.and_then(|metadata| metadata.modified().ok())
                    .and_then(unix_seconds)
            }),
        "accessed" => stat
            .and_then(|metadata| metadata.accessed().ok())
            .and_then(unix_seconds),
        "changed" => changed_seconds(stat?),
        _ => return None,
    }?;
    Some(timestamp_text(seconds))
}

fn unix_seconds(value: std::time::SystemTime) -> Option<i64> {
    value
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs() as i64)
}

#[cfg(unix)]
fn changed_seconds(metadata: &fs::Metadata) -> Option<i64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ctime())
}

#[cfg(not(unix))]
fn changed_seconds(_metadata: &fs::Metadata) -> Option<i64> {
    None
}

fn is_hidden(path: &Path, stat: Option<&fs::Metadata>) -> bool {
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
    {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return stat.is_some_and(|metadata| metadata.file_attributes() & 0x2 != 0);
    }
    #[cfg(not(windows))]
    {
        let _ = stat;
        false
    }
}

#[cfg(unix)]
fn filesystem_value(
    id: &str,
    stat: Option<&fs::Metadata>,
    _localization: &Localization,
) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = stat?;
    match id {
        "permissions" => Some(format!("{:04o}", metadata.mode() & 0o7777)),
        "owner" => Some(metadata.uid().to_string()),
        "group" => Some(metadata.gid().to_string()),
        "fileId" => Some(format!("{}:{}", metadata.dev(), metadata.ino())),
        "hardLinks" => Some(metadata.nlink().to_string()),
        "allocatedSize" => Some(size_text(metadata.blocks().saturating_mul(512))),
        _ => None,
    }
}

#[cfg(not(unix))]
fn filesystem_value(
    id: &str,
    stat: Option<&fs::Metadata>,
    localization: &Localization,
) -> Option<String> {
    let metadata = stat?;
    match id {
        "permissions" => Some(localization.text(if metadata.permissions().readonly() {
            "只读"
        } else {
            "可读写"
        })),
        _ => None,
    }
}

fn directory_stat_value(
    path: &Path,
    field: &str,
    cancelled: &AtomicBool,
) -> Result<String, String> {
    let direct = matches!(field, "directFiles" | "directDirectories" | "directOther");
    let mut count = 0_u64;
    let mut bytes = 0_u64;
    let mut stack = vec![PathBuf::from(path)];
    while let Some(directory) = stack.pop() {
        if cancelled.load(Ordering::Acquire) {
            return Err("Info calculation cancelled".into());
        }
        for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
            if cancelled.load(Ordering::Acquire) {
                return Err("Info calculation cancelled".into());
            }
            let entry = entry.map_err(|error| error.to_string())?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
            let file_type = metadata.file_type();
            if direct {
                let matches = match field {
                    "directFiles" => file_type.is_file(),
                    "directDirectories" => file_type.is_dir(),
                    "directOther" => !file_type.is_file() && !file_type.is_dir(),
                    _ => false,
                };
                if matches {
                    count += 1;
                }
            } else if file_type.is_symlink() {
                if field == "symbolicLinks" {
                    count += 1;
                }
            } else if file_type.is_dir() {
                if field == "recursiveDirectories" {
                    count += 1;
                }
                stack.push(entry.path());
            } else if file_type.is_file() {
                match field {
                    "recursiveFiles" => count += 1,
                    "recursiveSize" => bytes = bytes.saturating_add(metadata.len()),
                    "recursiveAllocatedSize" => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::MetadataExt;
                            bytes = bytes.saturating_add(metadata.blocks().saturating_mul(512));
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = &metadata;
                        }
                    }
                    _ => {}
                }
            }
        }
        if direct {
            break;
        }
    }
    Ok(match field {
        "recursiveSize" | "recursiveAllocatedSize" => size_text(bytes),
        _ => count.to_string(),
    })
}
