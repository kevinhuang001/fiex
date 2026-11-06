use crate::core::tags::{read_tags, Tag};
use fiex_resource::StorageProjection;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub extension: String,
    pub mime: String,
    pub link_target: Option<String>,
    /// Provider operations this entry allows. Empty grants no provider operations.
    #[serde(default)]
    pub allowed_ops: Vec<String>,
    pub tags: Vec<Tag>,
    /// Whether this entry, or a directory descendant, has cached bytes.
    #[serde(default)]
    pub has_cache: bool,
    /// Connection entries pinned with "Always keep locally".
    #[serde(default)]
    pub pinned: bool,
    /// Core-validated source storage identity for virtual projections. This
    /// remains process-local metadata and never becomes part of the UI DTO.
    #[serde(skip)]
    pub storage_projection: StorageProjection,
    /// The current bytes of this connection entry reached the remote.
    #[serde(default)]
    pub uploaded: bool,
    pub sync: Option<String>,
}

/// Operations supported by the local filesystem provider for a node shape.
/// Host permissions are checked by the operation itself, so this list does not
/// probe ACLs or file attributes while painting a directory.
fn local_operations(path: &str, is_dir: bool) -> Vec<String> {
    let mut operations: Vec<&str> = if is_dir {
        vec![
            "list", "stat", "search", "write", "mkdir", "rename", "remove",
        ]
    } else {
        vec!["stat", "read", "write", "rename", "remove"]
    };
    if crate::core::volumes::application_policy::protected(path) {
        operations
            .retain(|operation| !matches!(*operation, "write" | "mkdir" | "rename" | "remove"));
    }
    operations.into_iter().map(str::to_owned).collect()
}

pub fn entry(path: &Path) -> Result<Entry, String> {
    crate::plugins::mounts::reject_local(&path.to_string_lossy())?;
    let metadata = fiex_local_fs::inspect(path)?;
    Ok(Entry {
        path: path.to_string_lossy().into_owned(),
        name: metadata.name,
        is_dir: metadata.is_dir,
        is_symlink: metadata.is_symlink,
        size: metadata.size,
        modified: metadata.modified,
        created: metadata.created,
        extension: metadata.extension,
        mime: metadata.mime,
        link_target: metadata.link_target,
        allowed_ops: local_operations(&path.to_string_lossy(), metadata.is_dir),
        tags: read_tags(path),
        has_cache: false,
        pinned: false,
        uploaded: false,
        sync: None,
        storage_projection: StorageProjection::Owned,
    })
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// Operations supported by the directory itself, independent of current
    /// host ACLs. Mutations still report the operating system's real error.
    #[serde(default)]
    pub allowed_ops: Vec<String>,
    pub virtual_volume: bool,
    pub path: String,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
    /// Preferred initial view for this folder when no per-folder state exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_view: Option<String>,
}
pub fn list(path: &str, hidden: bool, ignores: &[String]) -> Result<Listing, String> {
    crate::plugins::mounts::reject_local(path)?;
    let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
    let ignore = crate::core::ignores::IgnoreSet::new(ignores);
    let mut paths = Vec::new();
    let mut warnings = Vec::new();
    for item in fs::read_dir(&path).map_err(|e| e.to_string())? {
        match item {
            Ok(item) => {
                if !hidden && item.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                if ignore.is_ignored(std::path::Path::new(&item.file_name())) {
                    continue;
                }
                paths.push(item.path());
            }
            Err(e) => warnings.push(e.to_string()),
        }
    }
    // Metadata, shortcut resolution and Finder tag xattrs are independent per
    // entry. Build large directories in parallel instead of serializing one
    // stat per row; small directories stay on the cheap sequential path.
    let results: Vec<Result<Entry, String>> = if paths.len() >= 32 {
        paths.par_iter().map(|path| entry(path)).collect()
    } else {
        paths.iter().map(|path| entry(path)).collect()
    };
    let mut entries = Vec::with_capacity(results.len());
    for result in results {
        match result {
            Ok(entry) => entries.push(entry),
            Err(error) => warnings.push(error),
        }
    }
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    Ok(Listing {
        allowed_ops: local_operations(&path.to_string_lossy(), true),
        virtual_volume: false,
        path: path.to_string_lossy().into_owned(),
        entries,
        warnings,
        default_view: None,
    })
}
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) {
        return Err(crate::i18n::t("名称不能为空，或包含路径分隔符").into());
    }
    #[cfg(target_os = "windows")]
    {
        let stem = name.split('.').next().unwrap_or("").to_uppercase();
        if name.contains(['<', '>', ':', '"', '|', '?', '*'])
            || name.ends_with([' ', '.'])
            || [
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9",
            ]
            .contains(&stem.as_str())
        {
            return Err(crate::i18n::t("此名称不能用于 Windows 文件").into());
        }
    }
    Ok(())
}
pub fn text_preview(path: &str) -> Result<String, String> {
    crate::plugins::mounts::reject_local(path)?;
    use std::io::Read;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(256 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.contains(&0) {
        return Err(crate::i18n::t("此文件为二进制格式，请使用默认应用打开").into());
    }
    String::from_utf8(bytes)
        .map_err(|_| crate::i18n::t("文本不是 UTF-8 编码，请使用默认应用打开").into())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn symlinks_report_their_target() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("Target.txt");
        std::fs::write(&target, "x").unwrap();
        let link = temp.path().join("Link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let entry = super::entry(&link).unwrap();
        assert!(entry.is_symlink);
        assert_eq!(
            entry.link_target.as_deref(),
            Some(target.to_string_lossy().as_ref())
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_tags_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Tagged.txt");
        std::fs::write(&file, "x").unwrap();
        let tags = vec![super::Tag {
            name: "TestTag".into(),
            color: 6,
        }];
        crate::core::tags::set_tags(&[file.to_string_lossy().into_owned()], &tags).unwrap();
        let entry = super::entry(&file).unwrap();
        assert!(entry
            .tags
            .iter()
            .any(|tag| tag.name == "TestTag" && tag.color == 6));
        crate::core::tags::set_tags(&[file.to_string_lossy().into_owned()], &[]).unwrap();
        assert!(super::entry(&file).unwrap().tags.is_empty());
    }
}
