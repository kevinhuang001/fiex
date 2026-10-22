use crate::files::validate_name;
use crate::ignores::IgnoreSet;
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// Error returned by every long operation once the shared flag is set.
pub const CANCELLED: &str = "已取消";

/// Shared cancel switch for the long-running operations of one app instance.
/// `begin_operation` resets it; `cancel_operation` flips it; the loops below
/// poll it and bail out with `CANCELLED` at a safe point.
#[derive(Clone, Default)]
pub struct OperationCancel(Arc<AtomicBool>);

impl OperationCancel {
    pub fn begin(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.0.clone()
    }
}

pub(crate) fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.into());
    }
    Ok(())
}

/// Copy a stream in chunks so a large file can be interrupted regularly.
pub(crate) fn copy_stream(
    input: &mut impl Read,
    output: &mut impl Write,
    cancel: &AtomicBool,
) -> Result<u64, String> {
    let mut buffer = vec![0u8; 256 * 1024];
    let mut total = 0;
    loop {
        check_cancelled(cancel)?;
        let read = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            return Ok(total);
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
        total += read as u64;
    }
}

#[derive(Debug)]
pub(crate) enum Undo {
    Rename(PathBuf, PathBuf),
    Created(PathBuf),
    Replaced(PathBuf, tempfile::TempDir),
}
#[derive(Default)]
pub struct Operations {
    undo: Mutex<Vec<Vec<Undo>>>,
}
#[derive(Default, Serialize)]
pub struct OperationResult {
    pub paths: Vec<String>,
    pub errors: Vec<String>,
    pub skipped: usize,
    pub conflicts: Vec<crate::transfers::Conflict>,
}
pub(crate) fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
pub fn unique(path: &Path) -> PathBuf {
    if !exists(path) {
        return path.into();
    }
    let stem = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem()
    }
    .unwrap_or_default()
    .to_string_lossy();
    let ext = (!path.is_dir())
        .then(|| path.extension())
        .flatten()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for i in 2.. {
        let next = path.with_file_name(format!("{stem} ({i}){ext}"));
        if !exists(&next) {
            return next;
        }
    }
    unreachable!()
}
pub(crate) fn copy_tree(
    src: &Path,
    dest: &Path,
    ignore: &IgnoreSet,
    relative: &Path,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    check_cancelled(cancel)?;
    if ignore.is_ignored(relative) {
        return Ok(1);
    }
    let meta = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        return Err("为避免复制到意外位置，请单独处理符号链接".into());
    }
    let mut skipped = 0;
    if meta.is_dir() {
        fs::create_dir(dest).map_err(|e| e.to_string())?;
        for item in fs::read_dir(src).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            skipped += copy_tree(
                &item.path(),
                &dest.join(item.file_name()),
                ignore,
                &relative.join(item.file_name()),
                cancel,
            )?;
        }
    } else if meta.is_file() {
        let mut input = fs::File::open(src).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(|e| e.to_string())?;
        copy_stream(&mut input, &mut output, cancel)?;
        fs::set_permissions(dest, meta.permissions()).map_err(|e| e.to_string())?;
    } else {
        return Err("不能复制设备或特殊文件".into());
    }
    Ok(skipped)
}
impl Operations {
    pub fn record_created(&self, path: PathBuf) {
        self.undo.lock().unwrap().push(vec![Undo::Created(path)]);
    }
    pub fn clear_undo(&self) {
        self.undo.lock().unwrap().clear();
    }

    pub fn create(&self, dir: &str, name: &str, folder: bool) -> Result<String, String> {
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
        self.undo
            .lock()
            .unwrap()
            .push(vec![Undo::Created(path.clone())]);
        Ok(path.to_string_lossy().into_owned())
    }
    pub fn rename(&self, source: &str, name: &str) -> Result<String, String> {
        validate_name(name)?;
        let src = Path::new(source);
        let dest = src.with_file_name(name);
        if src == dest {
            return Ok(source.into());
        }
        if exists(&dest) {
            return Err("同名项目已存在，请更换名称".into());
        }
        fs::rename(src, &dest).map_err(|e| e.to_string())?;
        self.undo
            .lock()
            .unwrap()
            .push(vec![Undo::Rename(dest.clone(), src.into())]);
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
        progress: &dyn Fn(u64, u64, Option<&str>),
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
pub fn recycle(paths: &[String]) -> Result<OperationResult, String> {
    let mut result = OperationResult {
        paths: vec![],
        errors: vec![],
        skipped: 0,
        conflicts: vec![],
    };
    for path in paths {
        let before = crate::volumes::trash_names();
        match trash::delete(path) {
            Ok(()) => {
                crate::volumes::record_trashed(path, &before);
                result.paths.push(path.clone());
            }
            Err(e) => result.errors.push(format!("{path}: {e}")),
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
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
    fn operation_cancel_flag_roundtrip() {
        let cancel = OperationCancel::default();
        assert!(!cancel.is_cancelled());
        cancel.cancel();
        assert!(cancel.is_cancelled());
        cancel.begin();
        assert!(!cancel.is_cancelled());
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
    fn copy_stream_stops_after_the_chunk_that_notices_cancellation() {
        struct Tripping<'a> {
            left: usize,
            cancel: &'a AtomicBool,
        }
        impl Read for Tripping<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let read = self.left.min(buffer.len());
                buffer[..read].fill(b'x');
                self.left -= read;
                // Pretend the user cancels while this read is in flight.
                self.cancel.store(true, Ordering::SeqCst);
                Ok(read)
            }
        }
        let cancel = AtomicBool::new(false);
        let mut input = Tripping {
            left: 4 * 1024 * 1024,
            cancel: &cancel,
        };
        let mut output = Vec::new();
        assert_eq!(
            copy_stream(&mut input, &mut output, &cancel).unwrap_err(),
            CANCELLED
        );
        // One chunk may land, but the copy stops long before the whole file.
        assert!(output.len() <= 256 * 1024);
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
