//! Create, rename, delete, write and cache write-back operations.

use super::mount::{ensure_access, ext4_mount};
use super::tree::{entry, read_path};
use super::util::{clean_path, join_rel, FsKind, Route};
use notify::Watcher;
use std::path::{Path, PathBuf};

fn ntfs_write(device: &str, path: &str, data: &[u8]) -> Result<(), String> {
    use fs_ntfs::facade::Filesystem;
    let fs = Filesystem::mount_rw(device).map_err(|error| error.to_string())?;
    let path = clean_path(path);
    if fs.stat(&path).is_err() {
        let (parent, name) = path.rsplit_once('/').ok_or("无效路径")?;
        fs.create_file(if parent.is_empty() { "/" } else { parent }, name)
            .map_err(|error| error.to_string())?;
    }
    fs.write_file_contents(&path, data)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn ext4_write(device: &str, path: &str, data: &[u8]) -> Result<(), String> {
    let fs = ext4_mount(device, true)?;
    let path = clean_path(path);
    if fs.lookup_path_bytes(path.as_bytes()).is_err() {
        fs.apply_create(&path, 0o100644)
            .map_err(|error| error.to_string())?;
    }
    fs.apply_pwrite(&path, 0, data)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn write(device: &str, kind: FsKind, path: &str, data: &[u8]) -> Result<(), String> {
    match kind {
        FsKind::Ntfs => ntfs_write(device, path, data),
        FsKind::Ext4 => ext4_write(device, path, data),
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => Err("该文件系统内置驱动为只读".into()),
    }
}

pub fn write_path(route: &Route, data: &[u8]) -> Result<(), String> {
    ensure_access(route)?;
    write(&route.driver_device, route.kind, &route.rel, data)
}

pub fn create(route: &Route, name: &str, folder: bool) -> Result<(), String> {
    ensure_access(route)?;
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("无效文件名".into());
    }
    let rel = join_rel(&route.rel, name);
    match route.kind {
        FsKind::Ntfs => {
            use fs_ntfs::facade::Filesystem;
            let fs = Filesystem::mount_rw(&route.driver_device).map_err(|e| e.to_string())?;
            if folder {
                fs.mkdir(&clean_path(&format!("/{rel}")), "")
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                fs.create_file("/".trim_end_matches('/'), &rel)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
        FsKind::Ext4 => {
            let fs = ext4_mount(&route.driver_device, true)?;
            let path = clean_path(&format!("/{rel}"));
            if folder {
                fs.apply_mkdir(&path, 0o40755)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                fs.apply_create(&path, 0o100644)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => Err("该文件系统内置驱动为只读".into()),
    }
}

pub fn remove(route: &Route) -> Result<(), String> {
    ensure_access(route)?;
    let path = clean_path(&format!("/{}", route.rel));
    match route.kind {
        FsKind::Ntfs => {
            use fs_ntfs::facade::Filesystem;
            let fs = Filesystem::mount_rw(&route.driver_device).map_err(|e| e.to_string())?;
            fs.remove(&path).map_err(|e| e.to_string())
        }
        FsKind::Ext4 => {
            let fs = ext4_mount(&route.driver_device, true)?;
            if fs
                .stat_ino(
                    fs.lookup_path_bytes(path.as_bytes())
                        .map_err(|e| e.to_string())?,
                )
                .map(|attrs| attrs.mode & 0o170000 == 0o040000)
                .unwrap_or(false)
            {
                fs.apply_rmdir(&path).map_err(|e| e.to_string())
            } else {
                fs.apply_unlink(&path).map_err(|e| e.to_string())
            }
        }
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => Err("该文件系统内置驱动为只读".into()),
    }
}

pub fn rename(route: &Route, new_name: &str) -> Result<(), String> {
    ensure_access(route)?;
    if new_name.contains('/') || new_name.contains('\\') || new_name == "." || new_name == ".." {
        return Err("无效文件名".into());
    }
    let path = clean_path(&format!("/{}", route.rel));
    match route.kind {
        FsKind::Ntfs => {
            use fs_ntfs::facade::Filesystem;
            let fs = Filesystem::mount_rw(&route.driver_device).map_err(|e| e.to_string())?;
            fs.rename(&path, new_name).map_err(|e| e.to_string())
        }
        FsKind::Ext4 => {
            let fs = ext4_mount(&route.driver_device, true)?;
            let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
            let target = clean_path(&format!("{parent}/{new_name}"));
            fs.apply_rename(&path, &target, false)
                .map_err(|e| e.to_string())
        }
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => Err("该文件系统内置驱动为只读".into()),
    }
}

fn exists(route: &Route) -> bool {
    entry(route).is_ok()
}

fn unique(route: &Route, name: &str) -> String {
    if !exists(&Route {
        rel: join_rel(&route.rel, name),
        ..route.clone()
    }) {
        return name.into();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.into());
    let ext = path
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    for index in 2.. {
        let candidate = format!("{stem} ({index}){ext}");
        if !exists(&Route {
            rel: join_rel(&route.rel, &candidate),
            ..route.clone()
        }) {
            return candidate;
        }
    }
    unreachable!()
}

pub fn copy_local_into(
    directory: &Route,
    source: &Path,
    keep_both: bool,
    moving: bool,
) -> Result<String, String> {
    let name = source
        .file_name()
        .ok_or("不能复制根目录")?
        .to_string_lossy()
        .into_owned();
    let name = if keep_both {
        unique(directory, &name)
    } else {
        name
    };
    let (copied, path) = copy_one(directory, source, &name)?;
    if moving && copied {
        if source.is_dir() {
            std::fs::remove_dir_all(source).map_err(|error| error.to_string())?;
        } else {
            std::fs::remove_file(source).map_err(|error| error.to_string())?;
        }
    }
    Ok(path)
}

fn copy_one(directory: &Route, source: &Path, name: &str) -> Result<(bool, String), String> {
    let meta = std::fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if meta.file_type().is_symlink() {
        return Err("不复制符号链接".into());
    }
    let child = Route {
        rel: join_rel(&directory.rel, name),
        ..directory.clone()
    };
    if meta.is_dir() {
        create(directory, name, true)?;
        for item in std::fs::read_dir(source).map_err(|error| error.to_string())? {
            let item = item.map_err(|error| error.to_string())?;
            let child_name = item.file_name().to_string_lossy().into_owned();
            copy_one(&child, &item.path(), &child_name)?;
        }
    } else if meta.is_file() {
        let data = std::fs::read(source).map_err(|error| error.to_string())?;
        write_path(&child, &data)?;
    } else {
        return Err("不能复制特殊文件".into());
    }
    Ok((
        true,
        format!("{}/{}", directory.root.trim_end_matches('/'), child.rel),
    ))
}

fn watchers() -> &'static std::sync::Mutex<Vec<notify::RecommendedWatcher>> {
    static WATCHERS: std::sync::OnceLock<std::sync::Mutex<Vec<notify::RecommendedWatcher>>> =
        std::sync::OnceLock::new();
    WATCHERS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Extracts a file to the local cache and watches it, writing edits back to the
/// raw filesystem whenever the external application saves.
pub fn open_cached(route: &Route) -> Result<String, String> {
    let data = read_path(route)?;
    let cache = dirs::home_dir()
        .ok_or("无法创建缓存目录")?
        .join(".fiex")
        .join("usbfs-cache")
        .join(Path::new(&route.root).file_name().unwrap_or_default())
        .join(&route.rel);
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&cache, data).map_err(|error| error.to_string())?;
    if matches!(route.kind, FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs) {
        return Ok(cache.to_string_lossy().into_owned());
    }
    fn digest(data: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }
    let watched = route.clone();
    let watched_file = cache.clone();
    let original = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(digest(&read_path(
        route,
    )?)));
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !matches!(
            event.kind,
            notify::EventKind::Modify(_) | notify::EventKind::Create(_)
        ) {
            return;
        }
        if !event.paths.iter().any(|path| path == &watched_file) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(220));
        let Ok(data) = std::fs::read(&watched_file) else {
            return;
        };
        let Ok(current) = read_path(&watched) else {
            return;
        };
        if digest(&current) != original.load(std::sync::atomic::Ordering::SeqCst) {
            let conflict = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".fiex")
                .join("usbfs-conflicts")
                .join(format!(
                    "{}-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|value| value.as_millis())
                        .unwrap_or_default(),
                    watched_file
                        .file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "file".into())
                ));
            if let Some(parent) = conflict.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(conflict, &data);
            return;
        }
        if write_path(&watched, &data).is_ok() {
            original.store(digest(&data), std::sync::atomic::Ordering::SeqCst);
        }
    })
    .map_err(|error| error.to_string())?;
    if let Some(parent) = cache.parent() {
        watcher
            .watch(parent, notify::RecursiveMode::NonRecursive)
            .map_err(|error| error.to_string())?;
    }
    watchers().lock().unwrap().push(watcher);
    Ok(cache.to_string_lossy().into_owned())
}
