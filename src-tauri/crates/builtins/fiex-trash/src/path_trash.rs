//! User Trash directory identities and confinement rules for Unix platforms.
#[cfg(windows)]
use crate::system_recycle_bin;
use crate::{RECYCLE_BIN_URI, TRASH_URI};
#[cfg(not(windows))]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use fiex_provider_contracts::{FsEntry, FsListing, FsOperation};
use std::path::PathBuf;

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
#[path = "path_helpers.rs"]
mod path_helpers;
#[cfg(all(test, not(windows)))]
#[path = "tests.rs"]
mod tests;
#[cfg(not(windows))]
use path_helpers::{confined, entry_for_path, io_error, relative, trash_path};
