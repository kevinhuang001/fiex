use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    pub color: u8,
}

#[derive(Clone, Debug, Serialize)]
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
    pub tags: Vec<Tag>,
}

#[cfg(target_os = "macos")]
const TAG_XATTR: &str = "com.apple.metadata:_kMDItemUserTags";

#[cfg(target_os = "macos")]
fn parse_tag(value: &str) -> Option<Tag> {
    let (name, color) = match value.rsplit_once('\n') {
        Some((name, color)) => (name, color.parse::<u8>().ok()?.min(7)),
        None => (value, 0),
    };
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some(Tag {
        name: name.to_string(),
        color,
    })
}

#[cfg(target_os = "macos")]
fn read_tags(path: &Path) -> Vec<Tag> {
    let Some(bytes) = xattr::get(path, TAG_XATTR).ok().flatten() else {
        return Vec::new();
    };
    let Ok(value) = plist::Value::from_reader(std::io::Cursor::new(bytes)) else {
        return Vec::new();
    };
    match value.as_array() {
        Some(items) => items
            .iter()
            .filter_map(|item| item.as_string())
            .filter_map(parse_tag)
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(not(target_os = "macos"))]
fn read_tags(_path: &Path) -> Vec<Tag> {
    Vec::new()
}

#[cfg(target_os = "macos")]
pub fn set_tags(paths: &[String], tags: &[Tag]) -> Result<(), String> {
    let mut values = Vec::new();
    for tag in tags {
        let name = tag.name.trim();
        if name.is_empty() || name.contains('\n') {
            continue;
        }
        values.push(plist::Value::String(format!(
            "{name}\n{}",
            tag.color.min(7)
        )));
    }
    let mut bytes = Vec::new();
    plist::Value::Array(values)
        .to_writer_binary(&mut bytes)
        .map_err(|e| e.to_string())?;
    for path in paths {
        let path = Path::new(path);
        if !path.exists() {
            continue;
        }
        if tags.is_empty() {
            match xattr::remove(path, TAG_XATTR) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        } else {
            xattr::set(path, TAG_XATTR, &bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_tags(_paths: &[String], _tags: &[Tag]) -> Result<(), String> {
    Err("Finder 标签仅在 macOS 上可用".into())
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
    let link = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = fs::metadata(path).unwrap_or(link.clone());
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
        tags: read_tags(path),
    })
}
#[derive(Serialize)]
pub struct Listing {
    pub path: String,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}
pub fn list(path: &str, hidden: bool, ignores: &[String]) -> Result<Listing, String> {
    let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
    let ignore = crate::ignores::IgnoreSet::new(ignores);
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
    entries.sort_unstable_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(Listing {
        path: path.to_string_lossy().into_owned(),
        entries,
        warnings,
    })
}
#[derive(Serialize)]
pub struct Location {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub available: u64,
    pub total: u64,
}
#[cfg(target_os = "macos")]
fn visible_mount(path: &str) -> bool {
    // APFS exposes the data volume and several synthetic volumes from the same
    // container. Only the root and user-mounted volumes should be listed.
    !path.starts_with("/System/Volumes/") && (path == "/" || path.starts_with("/Volumes/"))
}

pub fn locations() -> Vec<Location> {
    let mut out = Vec::new();
    let named: [(&str, Option<PathBuf>); 7] = [
        ("个人文件夹", dirs::home_dir()),
        ("桌面", dirs::desktop_dir()),
        ("下载", dirs::download_dir()),
        ("文稿", dirs::document_dir()),
        ("图片", dirs::picture_dir()),
        ("音乐", dirs::audio_dir()),
        ("视频", dirs::video_dir()),
    ];
    for (name, path) in named {
        if let Some(path) = path.filter(|p| p.exists()) {
            out.push(Location {
                name: name.into(),
                path: path.to_string_lossy().into_owned(),
                kind: "favorite".into(),
                available: 0,
                total: 0,
            });
        }
    }
    // macOS has a real Applications folder. Windows and Linux expose installed
    // applications through shell/desktop services, so those entries only open
    // the platform UI instead of pretending to be a browsable directory.
    let applications = if cfg!(target_os = "macos") {
        let path = PathBuf::from("/Applications");
        path.exists().then_some(path)
    } else {
        None
    };
    out.push(Location {
        name: "应用程序".into(),
        path: applications
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        kind: "applications".into(),
        available: 0,
        total: 0,
    });
    let mut seen = HashSet::new();
    for disk in sysinfo::Disks::new_with_refreshed_list().list() {
        let path = disk.mount_point().to_string_lossy().into_owned();
        #[cfg(target_os = "macos")]
        if !visible_mount(&path) {
            continue;
        }
        let mut name = disk.name().to_string_lossy().into_owned();
        if name.is_empty() {
            name = path.clone();
        }
        if !seen.insert((name.clone(), disk.total_space())) {
            continue;
        }
        out.push(Location {
            name,
            path,
            kind: "disk".into(),
            available: disk.available_space(),
            total: disk.total_space(),
        });
    }
    out
}

/// The user's Trash can be browsed like any other folder. Windows exposes the
/// Recycle Bin through a shell namespace instead of a directory, so it keeps
/// the system opener as a fallback.
pub fn trash_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|home| home.join(".Trash"))
            .filter(|path| path.is_dir())
    }
    #[cfg(target_os = "linux")]
    {
        dirs::data_dir()
            .map(|data| data.join("Trash/files"))
            .filter(|path| path.is_dir())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) {
        return Err("名称不能为空，或包含路径分隔符".into());
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
            return Err("此名称不能用于 Windows 文件".into());
        }
    }
    Ok(())
}
pub fn text_preview(path: &str) -> Result<String, String> {
    use std::io::Read;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(256 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.contains(&0) {
        return Err("此文件为二进制格式，请使用默认应用打开".into());
    }
    String::from_utf8(bytes).map_err(|_| "文本不是 UTF-8 编码，请使用默认应用打开".into())
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
        super::set_tags(&[file.to_string_lossy().into_owned()], &tags).unwrap();
        let entry = super::entry(&file).unwrap();
        assert!(entry
            .tags
            .iter()
            .any(|tag| tag.name == "TestTag" && tag.color == 6));
        super::set_tags(&[file.to_string_lossy().into_owned()], &[]).unwrap();
        assert!(super::entry(&file).unwrap().tags.is_empty());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_hides_synthetic_apfs_volumes() {
        assert!(super::visible_mount("/"));
        assert!(super::visible_mount("/Volumes/External"));
        assert!(!super::visible_mount("/System/Volumes/Data"));
        assert!(!super::visible_mount("/System/Volumes/VM"));
    }
}
