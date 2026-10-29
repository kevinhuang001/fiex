//! Read-only listing, lookup, search and copy-out operations.

use super::mount::{btrfs_fs, ensure_access, ext4_mount};
use super::util::{clean_path, join_rel, FsKind, Route, UsbEntry};
use super::write::remove;
use std::path::{Path, PathBuf};

mod drivers;
mod stream;
use drivers::*;
pub use stream::{copy_file_to, read_at};

pub fn list(device: &str, kind: FsKind, path: &str) -> Result<Vec<UsbEntry>, String> {
    let _guard = super::access::lock(device)?;
    if !Path::new(device).exists() {
        return Err(crate::i18n::t("设备不存在").into());
    }
    match kind {
        FsKind::Ntfs => ntfs_list(device, path),
        FsKind::Ext4 => ext4_list(device, path),
        FsKind::Apfs => apple_list(device, path, FsKind::Apfs),
        FsKind::Hfs => apple_list(device, path, FsKind::Hfs),
        FsKind::Btrfs => btrfs_list(device, path),
    }
}

pub fn read(device: &str, kind: FsKind, path: &str) -> Result<Vec<u8>, String> {
    let _guard = super::access::lock(device)?;
    match kind {
        FsKind::Ntfs => ntfs_read(device, path),
        FsKind::Ext4 => ext4_read(device, path),
        FsKind::Apfs => apple_read(device, path, FsKind::Apfs),
        FsKind::Hfs => apple_read(device, path, FsKind::Hfs),
        FsKind::Btrfs => btrfs_read(device, path),
    }
}

pub fn read_path(route: &Route) -> Result<Vec<u8>, String> {
    ensure_access(route)?;
    read(&route.driver_device, route.kind, &route.rel)
}

pub fn listing(route: &Route) -> Result<Vec<crate::core::files::Entry>, String> {
    ensure_access(route)?;
    let files = list(&route.driver_device, route.kind, &route.rel)?;
    Ok(files
        .into_iter()
        .map(|file| {
            let rel = join_rel(&route.rel, &file.name);
            let path = format!("{}/{}", route.root.trim_end_matches('/'), rel);
            crate::core::files::Entry {
                path,
                extension: Path::new(&file.name)
                    .extension()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                mime: mime_guess::from_path(&file.name)
                    .first_or_octet_stream()
                    .to_string(),
                name: file.name,
                is_dir: file.is_dir,
                is_symlink: false,
                size: file.size,
                modified: file.modified,
                created: file.modified,
                readonly: file.readonly,
                link_target: None,
                tags: vec![],
                ..Default::default()
            }
        })
        .collect())
}

pub fn entry(route: &Route) -> Result<crate::core::files::Entry, String> {
    if route.rel.is_empty() {
        ensure_access(route)?;
        return Ok(crate::core::files::Entry {
            path: route.root.clone(),
            name: "USB".into(),
            is_dir: true,
            readonly: matches!(route.kind, FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs),
            ..Default::default()
        });
    }
    let (parent, name) = route
        .rel
        .rsplit_once('/')
        .unwrap_or(("", route.rel.as_str()));
    let parent_route = Route {
        rel: parent.into(),
        ..route.clone()
    };
    let entries = listing(&parent_route)?;
    entries
        .into_iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| crate::i18n::t("文件不存在").to_string())
}

pub fn search(
    route: &Route,
    query: &str,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<crate::core::files::Entry>, bool), String> {
    let needle = query.to_lowercase();
    let mut out = Vec::new();
    let mut pending = vec![route.clone()];
    while let Some(directory) = pending.pop() {
        if cancelled() {
            return Ok((Vec::new(), false));
        }
        for entry in listing(&directory)? {
            if entry.name.to_lowercase().contains(&needle) {
                out.push(entry.clone());
                if out.len() >= limit {
                    return Ok((out, true));
                }
            }
            if entry.is_dir {
                pending.push(Route {
                    rel: join_rel(&directory.rel, &entry.name),
                    ..directory.clone()
                });
            }
        }
    }
    Ok((out, false))
}

fn local_unique(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "item".into());
    let ext = path
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for index in 2.. {
        let candidate = parent.join(format!("{stem} ({index}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn copy_route(source: &Route, target: &Path) -> Result<(), String> {
    let route_parent = source
        .rel
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let name = source
        .rel
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(source.rel.as_str());
    let entries = list(&source.driver_device, source.kind, route_parent)?;
    if let Some(entry) = entries.into_iter().find(|entry| entry.name == name) {
        if entry.is_dir {
            std::fs::create_dir_all(target).map_err(|error| error.to_string())?;
            for child in list(&source.driver_device, source.kind, &source.rel)? {
                let child_route = Route {
                    rel: join_rel(&source.rel, &child.name),
                    ..source.clone()
                };
                copy_route(&child_route, &target.join(child.name))?;
            }
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let mut output = std::fs::File::create(target).map_err(|e| e.to_string())?;
            copy_file_to(source, &mut output)?;
            output.sync_all().map_err(|e| e.to_string())?;
        }
        Ok(())
    } else {
        Err(crate::i18n::t("文件不存在").into())
    }
}

pub fn copy_to_local(
    source: &Route,
    destination: &Path,
    keep_both: bool,
    moving: bool,
) -> Result<String, String> {
    ensure_access(source)?;
    let name = source
        .rel
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(source.rel.as_str());
    if name.is_empty() {
        return Err(crate::i18n::t("不能复制整个虚拟卷根目录").into());
    }
    let target = if keep_both {
        local_unique(&destination.join(name))
    } else {
        let target = destination.join(name);
        if target.exists() {
            return Err(crate::i18n::t("目标位置已有同名项目").into());
        }
        target
    };
    copy_route(source, &target)?;
    if moving {
        remove(source)?;
    }
    Ok(target.to_string_lossy().into_owned())
}
