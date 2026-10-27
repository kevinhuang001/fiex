use crate::core::tags::{read_tags, Tag};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::UNIX_EPOCH};

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
    pub readonly: bool,
    pub link_target: Option<String>,
    /// Provider operations this entry allows. Empty means unrestricted.
    #[serde(default)]
    pub allowed_ops: Vec<String>,
    pub tags: Vec<Tag>,
    /// Whether this entry, or a directory descendant, has cached bytes.
    #[serde(default)]
    pub has_cache: bool,
    /// Connection entries pinned with "Always keep locally".
    #[serde(default)]
    pub pinned: bool,
    /// The current bytes of this connection entry reached the remote.
    #[serde(default)]
    pub uploaded: bool,
    pub sync: Option<String>,
}

#[cfg(target_os = "windows")]
fn shortcut_target(path: &Path) -> Option<String> {
    let link = lnk::ShellLink::open(path).ok()?;
    if let Some(info) = link.link_info() {
        let base = info
            .local_base_path_unicode()
            .as_ref()
            .or_else(|| info.local_base_path().as_ref())?;
        let suffix = info
            .common_path_suffix_unicode()
            .as_ref()
            .map(String::as_str)
            .filter(|suffix| !suffix.is_empty())
            .unwrap_or_else(|| info.common_path_suffix().as_str());
        return Some(format!("{base}{suffix}"));
    }
    link.relative_path().clone()
}

pub fn entry(path: &Path) -> Result<Entry, String> {
    crate::plugins::mounts::reject_local(&path.to_string_lossy())?;
    let link = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // Most entries are not links. Reuse lstat instead of issuing a second
    // metadata syscall for every item in a large directory.
    let meta = if link.file_type().is_symlink() {
        fs::metadata(path).unwrap_or(link.clone())
    } else {
        link.clone()
    };
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let link_target = if link.file_type().is_symlink() {
        fs::read_link(path)
            .ok()
            .map(|target| target.to_string_lossy().into_owned())
    } else {
        #[cfg(target_os = "windows")]
        {
            if extension == "lnk" {
                shortcut_target(path)
            } else {
                None
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    };
    Ok(Entry {
        path: path.to_string_lossy().into_owned(),
        name: path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        is_dir: meta.is_dir(),
        is_symlink: link.file_type().is_symlink(),
        size: meta.len(),
        modified: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |t| t.as_secs()),
        created: meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |t| t.as_secs()),
        extension,
        mime: mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string(),
        readonly: meta.permissions().readonly(),
        link_target,
        allowed_ops: Vec::new(),
        tags: read_tags(path),
        has_cache: false,
        pinned: false,
        uploaded: false,
        sync: None,
    })
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub read_only: bool,
    /// The directory itself accepts `mkdir` (provider/node capability).
    #[serde(default)]
    pub can_create: bool,
    pub virtual_volume: bool,
    pub path: String,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}
pub fn list(path: &str, hidden: bool, ignores: &[String]) -> Result<Listing, String> {
    crate::plugins::mounts::reject_local(path)?;
    let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
    let ignore = crate::core::ignores::IgnoreSet::new(ignores);
    let mut entries = Vec::new();
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
                match entry(&item.path()) {
                    Ok(e) => entries.push(e),
                    Err(e) => warnings.push(e),
                }
            }
            Err(e) => warnings.push(e.to_string()),
        }
    }
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    // Local creation follows the directory's own permission; the OS still
    // rejects a call on a read-only volume.
    let can_create = fs::metadata(&path).is_ok_and(|meta| !meta.permissions().readonly());
    Ok(Listing {
        read_only: false,
        can_create,
        virtual_volume: false,
        path: path.to_string_lossy().into_owned(),
        entries,
        warnings,
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
