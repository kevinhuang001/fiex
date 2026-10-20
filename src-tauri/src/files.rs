use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

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
}
pub fn entry(path: &Path) -> Result<Entry, String> {
    let link = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = fs::metadata(path).unwrap_or(link.clone());
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
        extension: path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase(),
        mime: mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string(),
        readonly: meta.permissions().readonly(),
    })
}
#[derive(Serialize)]
pub struct Listing {
    pub path: String,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}
pub fn list(path: &str, hidden: bool) -> Result<Listing, String> {
    let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for item in fs::read_dir(&path).map_err(|e| e.to_string())? {
        match item {
            Ok(item) => {
                if !hidden && item.file_name().to_string_lossy().starts_with('.') {
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
pub fn locations() -> Vec<Location> {
    let mut out = Vec::new();
    let named: [(&str, Option<PathBuf>); 8] = [
        ("个人文件夹", dirs::home_dir()),
        ("桌面", dirs::desktop_dir()),
        ("下载", dirs::download_dir()),
        ("文稿", dirs::document_dir()),
        ("图片", dirs::picture_dir()),
        ("音乐", dirs::audio_dir()),
        ("视频", dirs::video_dir()),
        (
            "应用程序",
            if cfg!(target_os = "macos") {
                Some(PathBuf::from("/Applications"))
            } else {
                None
            },
        ),
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
    for disk in sysinfo::Disks::new_with_refreshed_list().list() {
        out.push(Location {
            name: disk.name().to_string_lossy().into_owned(),
            path: disk.mount_point().to_string_lossy().into_owned(),
            kind: "disk".into(),
            available: disk.available_space(),
            total: disk.total_space(),
        });
    }
    out
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
