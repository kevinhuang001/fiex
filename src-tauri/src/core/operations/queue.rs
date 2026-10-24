//! The operation queue: the undo stack and the `Operations` API that feeds it.

use super::{exists, fs_ops, OperationResult, Undo};
use std::{
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Mutex},
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

    pub fn rename(&self, source: &str, name: &str) -> Result<String, String> {
        let Some(dest) = fs_ops::rename_entry(source, name)? else {
            return Ok(source.into());
        };
        self.undo
            .lock()
            .unwrap()
            .push(vec![Undo::Rename(dest.clone(), source.into())]);
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
        resolutions: &[crate::transfers::Resolution],
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
        resolutions: &[crate::transfers::Resolution],
        cancel: &AtomicBool,
        progress: &(dyn Fn(u64, u64, Option<&str>) + Sync),
    ) -> Result<OperationResult, String> {
        let (result, undo) = crate::transfers::execute(
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
        let mut actions = stack.pop().ok_or("没有可撤销的操作")?;
        while let Some(action) = actions.pop() {
            let result = match &action {
                Undo::Rename(src, dest) => {
                    if exists(dest) {
                        Err("原位置已有同名项目，无法撤销".into())
                    } else {
                        fs::create_dir_all(dest.parent().ok_or("原路径无效")?)
                            .map_err(|e| e.to_string())?;
                        crate::transfers::move_tree(src, dest, &crate::transfers::NEVER_CANCELLED)
                            .map(|_| ())
                    }
                }
                // Put copies/new files in Trash on undo, preserving any intervening edits.
                Undo::Replaced(path, backup) => {
                    if exists(path) {
                        Err("目标已改变，无法恢复被替换文件".into())
                    } else {
                        fs::rename(backup.path().join("original"), path).map_err(|e| e.to_string())
                    }
                }
                Undo::Created(path) => trash::delete(path).map_err(|e| e.to_string()),
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
mod tests {
    use super::*;
    use crate::operations::{OperationCancel, CANCELLED};

    #[test]
    fn transfers_never_overwrite() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        let b = t.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("x"), "source").unwrap();
        fs::write(b.join("x"), "existing").unwrap();
        let op = Operations::default();
        assert_eq!(
            op.transfer(
                &[a.join("x").to_string_lossy().into()],
                b.to_str().unwrap(),
                false,
                false,
                &[],
                &AtomicBool::new(false)
            )
            .unwrap()
            .conflicts
            .len(),
            1
        );
        op.transfer(
            &[a.join("x").to_string_lossy().into()],
            b.to_str().unwrap(),
            false,
            true,
            &[],
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(b.join("x")).unwrap(), "existing");
        assert!(b.join("x (2)").exists());
    }

    #[test]
    fn recursive_and_traversal_rejected() {
        let t = tempfile::tempdir().unwrap();
        let op = Operations::default();
        assert!(op
            .create(t.path().to_str().unwrap(), "../escape", true)
            .is_err());
        assert!(op
            .transfer(
                &[t.path().to_string_lossy().into()],
                t.path().to_str().unwrap(),
                false,
                true,
                &[],
                &AtomicBool::new(false)
            )
            .is_err());
    }

    #[test]
    fn multiple_selected_files_copy_through_the_parallel_path() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("a");
        let dest = t.path().join("b");
        fs::create_dir(&src).unwrap();
        fs::create_dir(&dest).unwrap();
        let sources = (0..32)
            .map(|index| {
                let path = src.join(format!("file-{index}.txt"));
                fs::write(&path, format!("content-{index}")).unwrap();
                path.to_string_lossy().into_owned()
            })
            .collect::<Vec<_>>();
        let op = Operations::default();
        let result = op
            .transfer(
                &sources,
                dest.to_str().unwrap(),
                false,
                false,
                &[],
                &AtomicBool::new(false),
            )
            .unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.paths.len(), 32);
        for index in 0..32 {
            assert_eq!(
                fs::read_to_string(dest.join(format!("file-{index}.txt"))).unwrap(),
                format!("content-{index}")
            );
        }
    }

    #[test]
    fn cancelled_transfer_stops_before_moving_any_file() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        let b = t.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("one"), "first").unwrap();
        fs::write(a.join("two"), "second").unwrap();
        let sources = vec![
            a.join("one").to_string_lossy().into_owned(),
            a.join("two").to_string_lossy().into_owned(),
        ];
        let cancel = OperationCancel::default();
        cancel.cancel();
        let op = Operations::default();
        assert_eq!(
            op.transfer(
                &sources,
                b.to_str().unwrap(),
                true,
                false,
                &[],
                &cancel.flag()
            )
            .err(),
            Some(CANCELLED.to_string())
        );
        assert!(!b.join("one").exists());
        assert!(!b.join("two").exists());
        assert!(a.join("one").exists());
        assert!(a.join("two").exists());
    }

    #[test]
    fn rename_undo() {
        let t = tempfile::tempdir().unwrap();
        let op = Operations::default();
        fs::write(t.path().join("a"), "x").unwrap();
        op.rename(t.path().join("a").to_str().unwrap(), "b")
            .unwrap();
        op.undo().unwrap();
        assert!(t.path().join("a").exists());
    }
}
