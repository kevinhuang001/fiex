//! Filesystem copy and integrity helpers used by the recoverable migration.
use std::path::Path;

pub(super) fn asset_filename(asset_id: &str, extension: &str) -> String {
    if extension.is_empty() {
        asset_id.to_owned()
    } else {
        format!("{asset_id}.{extension}")
    }
}

pub(super) fn copy_node(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if metadata.file_type().is_symlink() {
        let link = std::fs::read_link(source).map_err(|error| error.to_string())?;
        create_symlink(&link, target, metadata.is_dir())?;
        return Ok(());
    }
    if metadata.is_dir() {
        std::fs::create_dir(target).map_err(|error| error.to_string())?;
        let mut children = std::fs::read_dir(source)
            .map_err(|error| error.to_string())?
            .map(|entry| entry.map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            copy_node(&child.path(), &target.join(child.file_name()))?;
        }
        std::fs::set_permissions(target, metadata.permissions())
            .map_err(|error| error.to_string())?;
    } else if metadata.is_file() {
        std::fs::copy(source, target).map_err(|error| error.to_string())?;
        std::fs::set_permissions(target, metadata.permissions())
            .map_err(|error| error.to_string())?;
    } else {
        return Err(format!("unsupported migration node: {}", source.display()));
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path, directory: bool) -> Result<(), String> {
    let _ = directory;
    std::os::unix::fs::symlink(target, link).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path, directory: bool) -> Result<(), String> {
    if directory {
        std::os::windows::fs::symlink_dir(target, link).map_err(|error| error.to_string())
    } else {
        std::os::windows::fs::symlink_file(target, link).map_err(|error| error.to_string())
    }
}

#[cfg(not(any(unix, windows)))]
fn create_symlink(_target: &Path, _link: &Path, _directory: bool) -> Result<(), String> {
    Err("symbolic links cannot be migrated on this platform".into())
}

pub(super) fn fingerprint(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    fn walk(root: &Path, current: &Path, digest: &mut Sha256) -> Result<(), String> {
        let metadata = std::fs::symlink_metadata(current).map_err(|error| error.to_string())?;
        let rel = current
            .strip_prefix(root)
            .unwrap_or(current)
            .to_string_lossy();
        digest.update(rel.as_bytes());
        if metadata.file_type().is_symlink() {
            digest.update(*b"l");
            digest.update(
                std::fs::read_link(current)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .as_bytes(),
            );
        } else if metadata.is_dir() {
            digest.update(*b"d");
            let mut children = std::fs::read_dir(current)
                .map_err(|error| error.to_string())?
                .map(|entry| entry.map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            children.sort_by_key(|entry| entry.file_name());
            for child in children {
                walk(root, &child.path(), digest)?;
            }
        } else if metadata.is_file() {
            digest.update(*b"f");
            digest.update(metadata.len().to_le_bytes());
            let mut file = std::fs::File::open(current).map_err(|error| error.to_string())?;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                use std::io::Read;
                let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
                if read == 0 {
                    break;
                }
                digest.update(&buffer[..read]);
            }
        }
        Ok(())
    }
    let mut digest = Sha256::new();
    walk(path, path, &mut digest)?;
    Ok(hex::encode(digest.finalize()))
}

pub(super) fn remove_node_if_present(path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let result = if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_copy_verification_covers_nested_assets_and_symlink_targets() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let app = source.path().join("Example.app");
        std::fs::create_dir(&app).unwrap();
        std::fs::write(app.join("binary"), b"application bytes").unwrap();
        let copied = destination.path().join("Example.app");
        copy_node(&app, &copied).unwrap();
        assert_eq!(fingerprint(&app).unwrap(), fingerprint(&copied).unwrap());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = source.path().join("link");
            symlink("missing-target", &link).unwrap();
            let copied_link = destination.path().join("link");
            copy_node(&link, &copied_link).unwrap();
            assert_eq!(
                fingerprint(&link).unwrap(),
                fingerprint(&copied_link).unwrap()
            );
        }
    }
}
