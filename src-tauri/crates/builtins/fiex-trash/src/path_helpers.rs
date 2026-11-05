//! Platform filesystem identity helpers for the user Trash directory.
use crate::TRASH_URI;
#[cfg(not(windows))]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
#[cfg(not(windows))]
use fiex_provider_contracts::{FsEntry, FsKind, FsOperation};
#[cfg(not(windows))]
use std::path::{Component, Path, PathBuf};

#[cfg(not(windows))]
pub(super) fn trash_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let root = dirs::home_dir().map(|home| home.join(".Trash"));
    #[cfg(target_os = "linux")]
    let root = dirs::data_dir().map(|data| data.join("Trash/files"));
    root.ok_or_else(|| "NotFound: cannot locate the system Trash".into())
}

#[cfg(not(windows))]
pub(super) fn relative(uri: &str) -> Result<PathBuf, String> {
    let encoded = uri
        .strip_prefix(TRASH_URI)
        .ok_or("InvalidVfsUri: invalid Trash identity")?;
    if encoded.is_empty() {
        return Ok(PathBuf::new());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "InvalidVfsUri: invalid Trash node")?;
    let text = String::from_utf8(bytes).map_err(|_| "InvalidVfsUri: invalid Trash node")?;
    let path = PathBuf::from(text);
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err("ScopeDenied: Trash node escapes its root".into());
    }
    Ok(path)
}

#[cfg(not(windows))]
pub(super) fn confined(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let target = root.join(relative);
    let mut current = root.to_path_buf();
    let components: Vec<_> = relative.components().collect();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        current.push(component.as_os_str());
        if std::fs::symlink_metadata(&current)
            .map_err(io_error)?
            .file_type()
            .is_symlink()
        {
            return Err("ScopeDenied: cannot traverse a Trash symlink".into());
        }
    }
    Ok(target)
}

#[cfg(not(windows))]
pub(super) fn entry_for_path(path: &Path, node_id: String) -> Result<FsEntry, String> {
    let link_metadata = std::fs::symlink_metadata(path).map_err(io_error)?;
    let is_symlink = link_metadata.file_type().is_symlink();
    let metadata = if is_symlink {
        std::fs::metadata(path).unwrap_or(link_metadata.clone())
    } else {
        link_metadata.clone()
    };
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned();
    let is_directory = metadata.is_dir() && !is_symlink;
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    let mime = mime_guess::from_path(path)
        .first()
        .map(|mime| mime.to_string());
    let allowed_ops = if is_directory {
        vec![
            FsOperation::Stat,
            FsOperation::List,
            FsOperation::Search,
            FsOperation::Remove,
        ]
    } else {
        vec![FsOperation::Stat, FsOperation::Remove]
    };
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_secs().min(i64::MAX as u64) as i64);
    Ok(FsEntry {
        node_id,
        name,
        kind: if is_symlink {
            FsKind::Symlink
        } else if is_directory {
            FsKind::Directory
        } else {
            FsKind::File
        },
        size: Some(metadata.len()),
        modified,
        extension,
        mime,
        link_target: None,
        allowed_ops: Some(allowed_ops),
        ..Default::default()
    })
}

#[cfg(not(windows))]
pub(super) fn io_error(error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::PermissionDenied || error.raw_os_error() == Some(1) {
        format!("PermissionDenied: {error}")
    } else {
        error.to_string()
    }
}
