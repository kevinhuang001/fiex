use std::path::{Path, PathBuf};

pub(crate) fn free_destination(destination: PathBuf) -> PathBuf {
    if path_exists(&destination) {
        let parent = destination
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let stem = destination
            .file_stem()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| "恢复的项目".into());
        let extension = destination
            .extension()
            .map(|value| format!(".{}", value.to_string_lossy()))
            .unwrap_or_default();
        for index in 1..10_000 {
            let candidate = parent.join(format!("{stem} ({index}){extension}"));
            if !path_exists(&candidate) {
                return candidate;
            }
        }
    }
    destination
}

fn path_exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

pub(crate) fn move_entry(from: &Path, to: &Path) -> Result<(), String> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_entry(from, to)?;
    remove_entry(from)
}

fn copy_entry(from: &Path, to: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(from).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return copy_symlink(from, to);
    }
    if metadata.is_dir() {
        std::fs::create_dir_all(to).map_err(|error| error.to_string())?;
        for entry in std::fs::read_dir(from).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(
        std::fs::read_link(from).map_err(|error| error.to_string())?,
        to,
    )
    .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn copy_symlink(_from: &Path, _to: &Path) -> Result<(), String> {
    Err("Unsupported: cross-volume symlink restore".into())
}

pub(crate) fn remove_entry(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let result = if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn cross_volume_copy_preserves_symlinks_without_following_them() {
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        let source = temp.path().join("source");
        let target = temp.path().join("restored");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret"), b"do not copy").unwrap();
        std::os::unix::fs::symlink(&outside, &source).unwrap();

        copy_entry(&source, &target).unwrap();

        assert!(
            std::fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_link(target).unwrap(), outside);
    }

    #[test]
    fn free_destination_does_not_replace_existing_files_or_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("note.txt");
        std::fs::write(&existing, b"keep").unwrap();

        let free = free_destination(existing.clone());

        assert_eq!(free.file_name().unwrap(), "note (1).txt");
        assert_eq!(std::fs::read(existing).unwrap(), b"keep");
    }
}
