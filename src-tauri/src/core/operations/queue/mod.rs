//! The operation queue: the undo stack and the `Operations` API that feeds it.

use super::{OperationResult, Undo, exists, fs_ops};
use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, atomic::AtomicBool},
};

#[derive(Default)]
pub struct Operations {
    undo: Mutex<Vec<Vec<Undo>>>,
}

impl Operations {
    pub fn record_created(&self, path: PathBuf) {
        self.undo.lock().unwrap().push(vec![Undo::Created(path)]);
    }

    pub fn clear_undo(&self) {
        self.undo.lock().unwrap().clear();
    }

    pub fn create(&self, dir: &str, name: &str, folder: bool) -> Result<String, String> {
        let path = fs_ops::create_entry(dir, name, folder)?;
        self.record_created(path.clone());
        Ok(path.to_string_lossy().into_owned())
    }

    pub fn record_rename(&self, source: PathBuf, destination: PathBuf) {
        self.undo
            .lock()
            .unwrap()
            .push(vec![Undo::Rename(destination, source)]);
    }

    pub fn rename(&self, source: &str, name: &str) -> Result<String, String> {
        let Some(dest) = fs_ops::rename_entry(source, name)? else {
            return Ok(source.into());
        };
        self.record_rename(source.into(), dest.clone());
        Ok(dest.to_string_lossy().into_owned())
    }

    pub fn transfer(
        &self,
        sources: &[String],
        destination: &str,
        moving: bool,
        keep_both: bool,
        ignores: &[String],
        cancel: &AtomicBool,
    ) -> Result<OperationResult, String> {
        self.transfer_resolved_with_progress(
            sources,
            destination,
            moving,
            keep_both,
            ignores,
            &[],
            cancel,
            &|_: u64, _: u64, _: Option<&str>| {},
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn transfer_resolved(
        &self,
        sources: &[String],
        destination: &str,
        moving: bool,
        keep_both: bool,
        ignores: &[String],
        resolutions: &[crate::core::transfers::Resolution],
        cancel: &AtomicBool,
    ) -> Result<OperationResult, String> {
        self.transfer_resolved_with_progress(
            sources,
            destination,
            moving,
            keep_both,
            ignores,
            resolutions,
            cancel,
            &|_: u64, _: u64, _: Option<&str>| {},
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn transfer_resolved_with_progress(
        &self,
        sources: &[String],
        destination: &str,
        moving: bool,
        keep_both: bool,
        ignores: &[String],
        resolutions: &[crate::core::transfers::Resolution],
        cancel: &AtomicBool,
        progress: &(dyn Fn(u64, u64, Option<&str>) + Sync),
    ) -> Result<OperationResult, String> {
        crate::plugins::mounts::reject_local(destination)?;
        for source in sources {
            crate::plugins::mounts::reject_local(source)?;
        }
        let (result, undo) = crate::core::transfers::execute(
            sources,
            destination,
            moving,
            keep_both,
            ignores,
            resolutions,
            cancel,
            progress,
        )?;
        if !undo.is_empty() {
            self.undo.lock().unwrap().push(undo);
        }
        Ok(result)
    }

    pub fn undo(&self) -> Result<(), String> {
        let mut stack = self.undo.lock().unwrap();
        let mut actions = stack.pop().ok_or(crate::i18n::t("没有可撤销的操作"))?;
        while let Some(action) = actions.pop() {
            let result = match &action {
                Undo::Rename(src, dest) => {
                    if exists(dest) {
                        Err(crate::i18n::t("原位置已有同名项目，无法撤销").into())
                    } else {
                        fs::create_dir_all(dest.parent().ok_or(crate::i18n::t("原路径无效"))?)
                            .map_err(|e| e.to_string())?;
                        crate::core::transfers::move_tree(
                            src,
                            dest,
                            &crate::core::transfers::NEVER_CANCELLED,
                        )
                        .map(|_| ())
                    }
                }
                // Put copies/new files in Trash on undo, preserving any intervening edits.
                Undo::Replaced(path, backup) => {
                    if exists(path) {
                        Err(crate::i18n::t("目标已改变，无法恢复被替换文件").into())
                    } else {
                        fs::rename(backup.path().join("original"), path).map_err(|e| e.to_string())
                    }
                }
                Undo::Created(path) => {
                    fiex_trash::move_to_trash(path).map_err(|error| error.to_string())
                }
            };
            if let Err(e) = result {
                actions.push(action);
                stack.push(actions);
                return Err(e);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
