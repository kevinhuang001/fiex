//! Local filesystem primitives for create, rename, and recycle operations.

use crate::core::files::validate_name;
use crate::core::{fs::path_utils::entry_exists, operation_types::OperationResult};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn create_entry(dir: &str, name: &str, folder: bool) -> Result<PathBuf, String> {
    crate::plugins::mounts::reject_local(dir)?;
    validate_name(name)?;
    let path = Path::new(dir).join(name);
    if folder {
        fs::create_dir(&path).map_err(|e| e.to_string())?;
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
    }
    Ok(path)
}

/// Rename in place. `None` means the name did not change, so there is nothing
/// to undo.
pub(super) fn rename_entry(source: &str, name: &str) -> Result<Option<PathBuf>, String> {
    crate::plugins::mounts::reject_local(source)?;
    validate_name(name)?;
    let src = Path::new(source);
    let dest = src.with_file_name(name);
    if src == dest {
        return Ok(None);
    }
    if entry_exists(&dest) {
        return Err(crate::i18n::t("同名项目已存在，请更换名称").into());
    }
    fs::rename(src, &dest).map_err(|e| e.to_string())?;
    Ok(Some(dest))
}

pub fn recycle(paths: &[String]) -> Result<OperationResult, String> {
    for path in paths {
        crate::plugins::mounts::reject_local(path)?;
    }
    let mut result = OperationResult::default();
    for path in paths {
        match fiex_trash::move_to_trash(std::path::Path::new(path)) {
            Ok(()) => result.paths.push(path.clone()),
            Err(error) => result.errors.push(format!("{path}: {error}")),
        }
    }
    Ok(result)
}
