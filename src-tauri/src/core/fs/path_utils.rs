//! Small local-path helpers shared by filesystem operations and transfers.

use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn entry_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

pub(crate) fn unique_copy_destination(path: &Path) -> PathBuf {
    if !entry_exists(path) {
        return path.into();
    }
    let is_dir = path.is_dir();
    let stem = if is_dir {
        path.file_name()
    } else {
        path.file_stem()
    }
    .unwrap_or_default()
    .to_string_lossy();
    let extension = (!is_dir)
        .then(|| path.extension())
        .flatten()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    for index in 1.. {
        let candidate = path.with_file_name(format!("{stem} ({index}){extension}"));
        if !entry_exists(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_copy_destination_preserves_file_extension() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("report.txt");
        fs::write(&path, "original").unwrap();

        assert_eq!(
            unique_copy_destination(&path),
            temp.path().join("report (1).txt")
        );
    }

    #[test]
    fn unique_copy_destination_numbers_directories_without_an_extension() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("folder");
        fs::create_dir(&path).unwrap();

        assert_eq!(
            unique_copy_destination(&path),
            temp.path().join("folder (1)")
        );
    }
}
