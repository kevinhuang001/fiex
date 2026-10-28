//! Shared types, virtual path mapping and small helpers.

use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsKind {
    Ntfs,
    Ext4,
    Apfs,
    Hfs,
    Btrfs,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub readonly: bool,
}

#[derive(Clone, Debug)]
pub struct Route {
    pub device: String,
    pub driver_device: String,
    pub kind: FsKind,
    pub rel: String,
    pub root: String,
}

/// Normalize a slash separated path inside a filesystem image.
pub(crate) fn clean_path(path: &str) -> String {
    let mut out = String::from("/");
    for part in path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
    {
        if part == ".." {
            out = out
                .rsplit_once('/')
                .map(|(parent, _)| {
                    if parent.is_empty() {
                        "/".into()
                    } else {
                        parent.into()
                    }
                })
                .unwrap_or_else(|| "/".into());
        } else {
            if !out.ends_with('/') {
                out.push('/');
            }
            out.push_str(part);
        }
    }
    out
}

pub(crate) fn join_rel(rel: &str, name: &str) -> String {
    if rel.is_empty() {
        name.into()
    } else {
        format!("{}/{}", rel.trim_matches('/'), name)
    }
}

fn hex_encode(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hex_decode(value: &str) -> Option<String> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let bytes = (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

pub fn virtual_root(device: &str, kind: FsKind) -> String {
    let base = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".fiex")
        .join("usbfs");
    let kind = match kind {
        FsKind::Ntfs => "ntfs",
        FsKind::Ext4 => "ext4",
        FsKind::Apfs => "apfs",
        FsKind::Hfs => "hfs",
        FsKind::Btrfs => "btrfs",
    };
    base.join(format!("{}-{kind}", hex_encode(device)))
        .to_string_lossy()
        .into_owned()
}

pub fn route(path: &str) -> Option<Route> {
    let base = dirs::home_dir()?
        .join(".fiex")
        .join("usbfs")
        .to_string_lossy()
        .into_owned();
    let suffix = path.strip_prefix(&base)?;
    if !suffix.is_empty() && !suffix.starts_with(['/', '\\']) {
        return None;
    }
    let relative = suffix.trim_start_matches(['/', '\\']);
    // A virtual path must never carry parent traversal, including Windows
    // backslashes on a non-Windows host. Clamp-free parsing fails closed.
    if relative
        .replace('\\', "/")
        .split('/')
        .any(|part| part == "." || part == "..")
    {
        return None;
    }
    let (id, rel) = relative
        .split_once('/')
        .or_else(|| relative.split_once('\\'))
        .unwrap_or((relative, ""));
    let rel = rel.replace('\\', "/");
    let (encoded, kind) = id.rsplit_once('-')?;
    let device = hex_decode(encoded)?;
    let kind = match kind {
        "ntfs" => FsKind::Ntfs,
        "ext4" => FsKind::Ext4,
        "apfs" => FsKind::Apfs,
        "hfs" => FsKind::Hfs,
        "btrfs" => FsKind::Btrfs,
        _ => return None,
    };
    let driver_device = if cfg!(target_os = "macos") {
        device.replacen("/dev/disk", "/dev/rdisk", 1)
    } else {
        device.clone()
    };
    Some(Route {
        device,
        driver_device,
        kind,
        rel,
        root: Path::new(&base).join(id).to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod routing_security_tests {
    use super::*;
    #[test]
    fn rejects_virtual_root_prefix_confusion_and_parent_traversal() {
        let root = virtual_root("/dev/fiex-test-device", FsKind::Ntfs);
        assert!(route(&root).is_some());
        assert!(route(&(root.clone() + "/safe.txt")).is_some());
        assert!(route(&(root.clone() + "/../outside")).is_none());
        assert!(route(&(root.clone() + "/nested\\..\\outside")).is_none());
        assert!(route(&(root + "Evil/surprise")).is_none());
    }
}
