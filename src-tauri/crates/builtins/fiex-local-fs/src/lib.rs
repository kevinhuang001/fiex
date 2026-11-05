//! Host filesystem metadata inspection for the Local FS provider.
//!
//! Provider authorization, protected-path policy, tags, and cache projection
//! stay in Core. This crate owns the OS metadata reads and platform shortcut
//! target extraction used to build local directory entries.

use std::{fs, path::Path, time::UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryMetadata {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub extension: String,
    pub mime: String,
    pub link_target: Option<String>,
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

/// Inspect a host path without following the final symlink for its identity.
/// The target metadata is used for display when a link resolves; a dangling
/// link remains visible with its own metadata and target text.
pub fn inspect(path: &Path) -> Result<EntryMetadata, String> {
    let link =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let is_symlink = link.file_type().is_symlink();
    // Most entries are not links. Reuse lstat instead of issuing a second
    // metadata syscall for every row in a large directory.
    let metadata = if is_symlink {
        fs::metadata(path).unwrap_or_else(|_| link.clone())
    } else {
        link.clone()
    };
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let link_target = if is_symlink {
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

    Ok(EntryMetadata {
        name: path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        is_dir: metadata.is_dir(),
        is_symlink,
        size: metadata.len(),
        modified: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |time| time.as_secs()),
        created: metadata
            .created()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |time| time.as_secs()),
        extension,
        mime: mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string(),
        link_target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_projects_file_metadata_and_extension() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.txt");
        fs::write(&path, "hello").unwrap();

        let entry = inspect(&path).unwrap();
        assert_eq!(entry.name, "notes.txt");
        assert!(!entry.is_dir);
        assert!(!entry.is_symlink);
        assert_eq!(entry.size, 5);
        assert_eq!(entry.extension, "txt");
        assert_eq!(entry.mime, "text/plain");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_keep_the_link_target_and_target_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("Target.txt");
        fs::write(&target, "target").unwrap();
        let link = directory.path().join("Link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let entry = inspect(&link).unwrap();
        assert!(entry.is_symlink);
        assert_eq!(entry.size, 6);
        assert_eq!(
            entry.link_target.as_deref(),
            Some(target.to_string_lossy().as_ref())
        );
    }
}
