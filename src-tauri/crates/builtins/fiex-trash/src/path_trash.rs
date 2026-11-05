//! User Trash directory identities and confinement rules for Unix platforms.
#[cfg(windows)]
use crate::system_recycle_bin;
use crate::{RECYCLE_BIN_URI, TRASH_URI};
#[cfg(not(windows))]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
#[cfg(not(windows))]
use fiex_provider_contracts::FsKind;
use fiex_provider_contracts::{FsEntry, FsListing, FsOperation};
use std::path::PathBuf;
#[cfg(not(windows))]
use std::path::{Component, Path};

#[derive(Default)]
pub struct TrashDirectory {
    #[cfg(not(windows))]
    root_override: Option<PathBuf>,
}

impl TrashDirectory {
    pub fn system() -> Self {
        Self::default()
    }

    #[cfg(not(windows))]
    fn root_path(&self) -> Result<PathBuf, String> {
        self.root_override
            .clone()
            .map(Ok)
            .unwrap_or_else(trash_path)
    }

    #[cfg(all(test, not(windows)))]
    fn with_root(root: PathBuf) -> Self {
        Self {
            root_override: Some(root),
        }
    }

    pub fn handles(&self, path: &str) -> bool {
        path.starts_with(TRASH_URI)
    }

    pub fn is_tree(&self, path: &str) -> bool {
        self.handles(path) || path == RECYCLE_BIN_URI
    }

    /// Check access before cached metadata is displayed so a revoked directory
    /// permission cannot make stale entries appear current.
    pub fn check_access(&self) -> Result<(), String> {
        #[cfg(not(windows))]
        {
            let root = self.root_path()?;
            match std::fs::read_dir(root) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(error)),
            }
        }
        Ok(())
    }

    pub fn physical(&self, uri: &str) -> Result<PathBuf, String> {
        #[cfg(not(windows))]
        {
            let root = self.root_path()?;
            confined(&root, &relative(uri)?)
        }
        #[cfg(windows)]
        {
            let _ = uri;
            Err("InvalidVfsUri: use the issued Recycle Bin node identity".into())
        }
    }

    pub fn action_path(&self, path: &str) -> Result<String, String> {
        if !self.handles(path) {
            return Ok(path.into());
        }
        if path == TRASH_URI {
            return Err("ScopeDenied: cannot mutate the Trash root".into());
        }
        #[cfg(not(windows))]
        {
            self.physical(path)
                .map(|path| path.to_string_lossy().into_owned())
        }
        #[cfg(windows)]
        {
            Err("InvalidVfsUri: use the issued Recycle Bin node identity".into())
        }
    }

    pub fn list(&self, uri: &str) -> Result<FsListing, String> {
        self.check_access()?;
        #[cfg(windows)]
        {
            let _ = uri;
            system_recycle_bin()
                .list()
                .map_err(|error| error.to_string())
        }
        #[cfg(not(windows))]
        {
            let root = self.root_path()?;
            let path = self.physical(uri)?;
            if std::fs::symlink_metadata(&path)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                return Err("ScopeDenied: cannot browse a Trash symlink".into());
            }
            if !path.exists() {
                return Ok(FsListing::default());
            }
            let mut entries = std::fs::read_dir(&path)
                .map_err(io_error)?
                .map(|entry| entry.map_err(io_error))
                .map(|entry| {
                    let entry = entry?;
                    let physical = entry.path();
                    let relative = physical
                        .strip_prefix(&root)
                        .map_err(|_| "ScopeDenied: Trash listing escaped its root")?;
                    let node_id = format!(
                        "{TRASH_URI}{}",
                        URL_SAFE_NO_PAD.encode(relative.to_string_lossy().as_bytes())
                    );
                    entry_for_path(&physical, node_id)
                })
                .collect::<Result<Vec<_>, String>>()?;
            entries.sort_by_key(|entry| entry.name.to_lowercase());
            Ok(FsListing {
                entries,
                ..Default::default()
            })
        }
    }

    pub fn validate_child(&self, parent: &str, child: &str) -> Result<(), String> {
        if self
            .list(parent)?
            .entries
            .iter()
            .any(|entry| entry.node_id == child)
        {
            Ok(())
        } else {
            Err("ScopeDenied: child is not listed by the selected Trash directory".into())
        }
    }

    pub fn purge(&self, uri: &str) -> Result<(), String> {
        if uri == TRASH_URI {
            return Err("ScopeDenied: Trash root cannot be permanently removed".into());
        }
        self.check_access()?;
        #[cfg(not(windows))]
        {
            let path = self.physical(uri)?;
            let metadata = std::fs::symlink_metadata(&path).map_err(io_error)?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                std::fs::remove_dir_all(path).map_err(io_error)
            } else {
                std::fs::remove_file(path).map_err(io_error)
            }
        }
        #[cfg(windows)]
        {
            let _ = uri;
            Err("InvalidVfsUri: use the issued Recycle Bin node identity".into())
        }
    }

    pub fn entry(&self, uri: &str) -> Result<FsEntry, String> {
        if uri == TRASH_URI {
            return Ok(FsEntry::directory(TRASH_URI, "Trash").allowing([
                FsOperation::List,
                FsOperation::Stat,
                FsOperation::Search,
            ]));
        }
        #[cfg(windows)]
        {
            system_recycle_bin()
                .list()
                .map_err(|error| error.to_string())?
                .entries
                .into_iter()
                .find(|entry| entry.node_id == uri)
                .ok_or_else(|| "NotFound: Trash node missing".into())
        }
        #[cfg(not(windows))]
        {
            let physical = self.physical(uri)?;
            entry_for_path(&physical, uri.to_owned())
        }
    }
}

#[cfg(not(windows))]
fn trash_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let root = dirs::home_dir().map(|home| home.join(".Trash"));
    #[cfg(target_os = "linux")]
    let root = dirs::data_dir().map(|data| data.join("Trash/files"));
    root.ok_or_else(|| "NotFound: cannot locate the system Trash".into())
}

#[cfg(not(windows))]
fn relative(uri: &str) -> Result<PathBuf, String> {
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
fn confined(root: &Path, relative: &Path) -> Result<PathBuf, String> {
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
fn entry_for_path(path: &Path, node_id: String) -> Result<FsEntry, String> {
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
fn io_error(error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::PermissionDenied || error.raw_os_error() == Some(1) {
        format!("PermissionDenied: {error}")
    } else {
        error.to_string()
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn trash_uris_reject_absolute_and_traversal_paths() {
        for value in ["../outside", "/etc/passwd", "a/../../outside"] {
            let uri = format!("{TRASH_URI}{}", URL_SAFE_NO_PAD.encode(value));
            assert!(relative(&uri).is_err());
        }
        assert_eq!(
            relative(&format!(
                "{TRASH_URI}{}",
                URL_SAFE_NO_PAD.encode("folder/file")
            ))
            .unwrap(),
            PathBuf::from("folder/file")
        );
    }

    #[test]
    fn directory_listing_uses_opaque_ids_and_node_capabilities() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Folder")).unwrap();
        std::fs::write(root.path().join("notes.txt"), b"notes").unwrap();
        let backend = TrashDirectory::with_root(root.path().to_path_buf());

        let listing = backend.list(TRASH_URI).unwrap();
        let file = listing
            .entries
            .iter()
            .find(|entry| entry.name == "notes.txt")
            .unwrap();
        let folder = listing
            .entries
            .iter()
            .find(|entry| entry.name == "Folder")
            .unwrap();
        assert!(file.node_id.starts_with(TRASH_URI));
        assert!(!file
            .node_id
            .contains(root.path().to_string_lossy().as_ref()));
        assert!(file.allows(FsOperation::Remove));
        assert!(!file.allows(FsOperation::List));
        assert!(folder.allows(FsOperation::List));
        backend.validate_child(TRASH_URI, &file.node_id).unwrap();
        assert_eq!(backend.entry(&file.node_id).unwrap().name, "notes.txt");
    }

    #[cfg(unix)]
    #[test]
    fn intermediate_trash_symlinks_cannot_escape_the_root() {
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/tmp", root.path().join("escape")).unwrap();
        assert!(confined(root.path(), Path::new("escape/child")).is_err());
    }
}
