//! The copy/move engine: walk the selection into a plan, resolve collisions
//! and only then touch the filesystem.

use super::{
    conflicts::{decide, Conflict, Decision, Resolution},
    progress::Progress,
};
use crate::{
    core::ignores::IgnoreSet,
    core::operations::{
        check_cancelled, copy_stream, copy_tree_with_progress, exists, unique, OperationResult,
        Undo, CANCELLED,
    },
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

mod installation;
pub(crate) use installation::*;

mod execution;
pub(crate) use execution::*;

#[cfg(test)]
mod tests;

struct Task {
    src: PathBuf,
    dest: PathBuf,
    replace: bool,
    files: u64,
}

fn count_files(path: &Path, ignore: &IgnoreSet, relative: &Path) -> Result<u64, String> {
    if ignore.is_ignored(relative) {
        return Ok(0);
    }
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        return Err(crate::i18n::t("暂不支持复制或移动符号链接及特殊文件").into());
    }
    if meta.is_file() {
        return Ok(1);
    }
    if !meta.is_dir() {
        return Ok(0);
    }
    let mut total = 0;
    for child in fs::read_dir(path).map_err(|e| e.to_string())? {
        let child = child.map_err(|e| e.to_string())?;
        total += count_files(&child.path(), ignore, &relative.join(child.file_name()))?;
    }
    Ok(total)
}

struct Plan<'a> {
    tasks: Vec<Task>,
    merged: Vec<PathBuf>,
    conflicts: Vec<Conflict>,
    reserved: HashSet<PathBuf>,
    resolutions: &'a [Resolution],
    ignore: IgnoreSet,
    skipped: usize,
    keep_both: bool,
    moving: bool,
    cancel: &'a AtomicBool,
}

impl Plan<'_> {
    fn walk(&mut self, src: PathBuf, mut dest: PathBuf, relative: PathBuf) -> Result<(), String> {
        check_cancelled(self.cancel)?;
        if self.ignore.is_ignored(&relative) {
            self.skipped += 1;
            return Ok(());
        }
        let meta = fs::symlink_metadata(&src).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
            return Err(crate::i18n::t("暂不支持复制或移动符号链接及特殊文件").into());
        }
        if exists(&dest) {
            let same = dunce::canonicalize(&src).ok() == dunce::canonicalize(&dest).ok();
            if same && self.moving {
                return Ok(());
            }
            if !same
                && meta.is_dir()
                && fs::symlink_metadata(&dest)
                    .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
                && !self.keep_both
            {
                self.merged.push(src.clone());
                for child in fs::read_dir(&src).map_err(|e| e.to_string())? {
                    let child = child.map_err(|e| e.to_string())?;
                    self.walk(
                        child.path(),
                        dest.join(child.file_name()),
                        relative.join(child.file_name()),
                    )?;
                }
                return Ok(());
            }
            match decide(&src, &dest, self.resolutions, self.keep_both)? {
                Decision::Pending(conflict) => {
                    self.conflicts.push(conflict);
                    return Ok(());
                }
                Decision::Skip => {
                    self.skipped += 1;
                    return Ok(());
                }
                Decision::KeepBoth => dest = unique(&dest),
                Decision::Replace => {
                    if same {
                        return Err(crate::i18n::t("不能用文件替换其自身，请选择保留两者").into());
                    }
                    self.reserve(&dest)?;
                    let files = count_files(&src, &self.ignore, &relative)?;
                    self.tasks.push(Task {
                        src,
                        dest,
                        replace: true,
                        files,
                    });
                    return Ok(());
                }
            }
        }
        // A duplicate name in one selection must never overwrite another task.
        self.reserve(&dest)?;
        let files = count_files(&src, &self.ignore, &relative)?;
        self.tasks.push(Task {
            src,
            dest,
            replace: false,
            files,
        });
        Ok(())
    }

    fn reserve(&mut self, path: &Path) -> Result<(), String> {
        if !self.reserved.insert(path.into()) {
            return Err(crate::i18n::t("所选项目有重复文件名，请分别操作").into());
        }
        Ok(())
    }
}
