use crate::{
    ignores::IgnoreSet,
    operations::{
        check_cancelled, copy_stream, copy_tree, exists, unique, OperationResult, Undo, CANCELLED,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};

/// Undo moves cannot be interrupted; this flag is never set.
pub(crate) static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVersion {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub is_dir: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Conflict {
    pub key: String,
    pub source: FileVersion,
    pub destination: FileVersion,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Replace,
    Skip,
    KeepBoth,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Resolution {
    pub key: String,
    pub action: Action,
}
fn version(path: &Path) -> Result<(FileVersion, u128), String> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let stamp = m
        .modified()
        .ok()
        .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    Ok((
        FileVersion {
            path: path.to_string_lossy().into_owned(),
            size: m.len(),
            modified: stamp.as_secs(),
            is_dir: m.is_dir(),
        },
        stamp.as_nanos(),
    ))
}
struct Task {
    src: PathBuf,
    dest: PathBuf,
    replace: bool,
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
            return Err("暂不支持复制或移动符号链接及特殊文件".into());
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
            let (source, source_stamp) = version(&src)?;
            let (destination, dest_stamp) = version(&dest)?;
            let key = format!(
                "{}\0{}\0{}\0{}\0{}\0{}",
                source.path,
                destination.path,
                source_stamp,
                dest_stamp,
                source.size,
                destination.size
            );
            let action = if self.keep_both {
                Some(Action::KeepBoth)
            } else {
                self.resolutions
                    .iter()
                    .find(|r| r.key == key)
                    .map(|r| r.action)
            };
            match action {
                None => {
                    self.conflicts.push(Conflict {
                        key,
                        source,
                        destination,
                    });
                    return Ok(());
                }
                Some(Action::Skip) => {
                    self.skipped += 1;
                    return Ok(());
                }
                Some(Action::KeepBoth) => dest = unique(&dest),
                Some(Action::Replace) => {
                    if same {
                        return Err("不能用文件替换其自身，请选择保留两者".into());
                    }
                    self.reserve(&dest)?;
                    self.tasks.push(Task {
                        src,
                        dest,
                        replace: true,
                    });
                    return Ok(());
                }
            }
        }
        // A duplicate name in one selection must never overwrite another task.
        self.reserve(&dest)?;
        self.tasks.push(Task {
            src,
            dest,
            replace: false,
        });
        Ok(())
    }
    fn reserve(&mut self, path: &Path) -> Result<(), String> {
        if !self.reserved.insert(path.into()) {
            return Err("所选项目有重复文件名，请分别操作".into());
        }
        Ok(())
    }
}
fn remove(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|e| e.to_string())
}

fn install_copy(
    src: &Path,
    dest: &Path,
    ignore: &IgnoreSet,
    cancel: &AtomicBool,
) -> Result<usize, String> {
    check_cancelled(cancel)?;
    let staging = tempfile::Builder::new()
        .prefix(".fiex-copy-")
        .tempdir_in(dest.parent().ok_or("目标路径无效")?)
        .map_err(|e| e.to_string())?;
    let staged = staging.path().join("incoming");
    let skipped = copy_tree(
        src,
        &staged,
        ignore,
        Path::new(src.file_name().unwrap_or_default()),
        cancel,
    )?;
    if staged.is_dir() {
        if exists(dest) {
            return Err("目标已改变，请重试".into());
        }
        fs::rename(&staged, dest).map_err(|e| e.to_string())?;
    } else {
        match fs::hard_link(&staged, dest) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err("目标已改变，请重试".into())
            }
            Err(_) => {
                let mut output = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(dest)
                    .map_err(|e| e.to_string())?;
                let copied = fs::File::open(&staged)
                    .and_then(|mut input| {
                        copy_stream(&mut input, &mut output, cancel).map_err(std::io::Error::other)
                    })
                    .and_then(|_| fs::set_permissions(dest, fs::metadata(&staged)?.permissions()));
                drop(output);
                if let Err(e) = copied {
                    let _ = fs::remove_file(dest);
                    return Err(e.to_string());
                }
            }
        }
    }
    Ok(skipped)
}
pub(crate) fn move_tree(src: &Path, dest: &Path, cancel: &AtomicBool) -> Result<usize, String> {
    check_cancelled(cancel)?;
    if exists(dest) {
        return Err("目标已改变，请重试".into());
    }
    if src.is_file() {
        match fs::hard_link(src, dest) {
            Ok(()) => {
                if let Err(e) = fs::remove_file(src) {
                    let _ = fs::remove_file(dest);
                    return Err(e.to_string());
                }
                return Ok(0);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err("目标已改变，请重试".into())
            }
            Err(_) => (),
        }
    } else {
        match fs::rename(src, dest) {
            Ok(()) => return Ok(0),
            Err(e) if e.kind() != std::io::ErrorKind::CrossesDevices => return Err(e.to_string()),
            Err(_) => (),
        }
    }
    install_copy(src, dest, &IgnoreSet::new(&[]), cancel)?;
    remove(src)?;
    Ok(0)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute(
    sources: &[String],
    destination: &str,
    moving: bool,
    keep_both: bool,
    ignores: &[String],
    resolutions: &[Resolution],
    cancel: &AtomicBool,
    progress: &dyn Fn(u64, u64, Option<&str>),
) -> Result<(OperationResult, Vec<Undo>), String> {
    check_cancelled(cancel)?;
    let dest_dir = dunce::canonicalize(destination).map_err(|e| e.to_string())?;
    if !dest_dir.is_dir() {
        return Err("目标不是文件夹".into());
    }
    let mut plan = Plan {
        tasks: vec![],
        merged: vec![],
        conflicts: vec![],
        reserved: HashSet::new(),
        resolutions,
        ignore: IgnoreSet::new(ignores),
        skipped: 0,
        keep_both,
        moving,
        cancel,
    };
    for source in sources {
        check_cancelled(cancel)?;
        let src = PathBuf::from(source);
        let real = dunce::canonicalize(&src).map_err(|e| e.to_string())?;
        if dest_dir.starts_with(&real) {
            return Err("不能把文件夹复制或移动到其自身内部".into());
        }
        let name = src.file_name().ok_or("不能操作根目录")?.to_os_string();
        plan.walk(src, dest_dir.join(&name), PathBuf::from(name))?;
    }
    let mut result = OperationResult {
        skipped: plan.skipped,
        conflicts: plan.conflicts,
        ..Default::default()
    };
    // Collect conflicts before touching any source or destination.
    if !result.conflicts.is_empty() {
        return Ok((result, vec![]));
    }
    let mut undo = vec![];
    let total = plan.tasks.len() as u64;
    let mut done = 0u64;
    for task in plan.tasks {
        // Cancellation stops at a file boundary; moved files are not rolled back.
        check_cancelled(cancel)?;
        let Task { src, dest, replace } = task;
        done += 1;
        progress(done, total, dest.file_name().and_then(|name| name.to_str()));
        // Re-check immediately before mutation: a new target must get a fresh dialog.
        if exists(&dest) && !replace {
            result
                .errors
                .push(format!("{}: 目标已改变，请重试", dest.display()));
            continue;
        }
        let backup = if replace {
            let storage = tempfile::Builder::new()
                .prefix(".fiex-undo-")
                .tempdir_in(dest.parent().ok_or("目标路径无效")?)
                .map_err(|e| e.to_string())?;
            if let Err(e) = fs::rename(&dest, storage.path().join("original")) {
                result.errors.push(e.to_string());
                continue;
            }
            Some(storage)
        } else {
            None
        };
        let operation = if moving {
            move_tree(&src, &dest, cancel)
        } else {
            install_copy(&src, &dest, &plan.ignore, cancel)
        };
        match operation {
            Ok(skipped) => {
                result.skipped += skipped;
                result.paths.push(dest.to_string_lossy().into_owned());
                if let Some(backup) = backup {
                    undo.push(Undo::Replaced(dest.clone(), backup));
                }
                undo.push(if moving {
                    Undo::Rename(dest, src)
                } else {
                    Undo::Created(dest)
                });
            }
            Err(e) => {
                let cancelled = cancel.load(Ordering::Relaxed);
                if let Some(backup) = backup {
                    if exists(&dest) {
                        if cancelled {
                            // The half-written copy belongs to us; drop it and
                            // put the replaced original back before bailing out.
                            let _ = remove(&dest);
                            fs::rename(backup.path().join("original"), &dest)
                                .map_err(|e| e.to_string())?;
                        } else {
                            let saved = backup.keep();
                            result
                                .errors
                                .push(format!("目标已改变，原文件保存在 {}", saved.display()));
                        }
                    } else {
                        fs::rename(backup.path().join("original"), &dest)
                            .map_err(|e| e.to_string())?;
                    }
                }
                if cancelled {
                    return Err(CANCELLED.into());
                }
                result.errors.push(format!("{}: {e}", src.display()));
            }
        }
    }
    if total > 0 {
        progress(total, total, None);
    }
    if moving {
        for src in plan.merged.into_iter().rev() {
            let _ = fs::remove_dir(src);
        }
    }
    Ok((result, undo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::Operations;
    fn resolve(result: &OperationResult, action: Action) -> Vec<Resolution> {
        result
            .conflicts
            .iter()
            .map(|c| Resolution {
                key: c.key.clone(),
                action,
            })
            .collect()
    }
    #[test]
    fn conflicts_are_preflighted_replace_is_undoable_and_stale_choices_are_rejected() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        let b = t.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("x.txt"), "source").unwrap();
        fs::write(a.join("new.txt"), "new").unwrap();
        fs::write(b.join("x.txt"), "original").unwrap();
        let sources = vec![
            a.join("x.txt").to_string_lossy().into_owned(),
            a.join("new.txt").to_string_lossy().into_owned(),
        ];
        let op = Operations::default();
        let first = op
            .transfer(
                &sources,
                b.to_str().unwrap(),
                true,
                false,
                &[],
                &NEVER_CANCELLED,
            )
            .unwrap();
        assert_eq!(first.conflicts.len(), 1);
        assert!(!b.join("new.txt").exists());
        let choices = resolve(&first, Action::Replace);
        fs::write(b.join("x.txt"), "changed since prompt").unwrap();
        let stale = op
            .transfer_resolved(
                &sources,
                b.to_str().unwrap(),
                true,
                false,
                &[],
                &choices,
                &NEVER_CANCELLED,
            )
            .unwrap();
        assert_eq!(stale.conflicts.len(), 1);
        let result = op
            .transfer_resolved(
                &sources,
                b.to_str().unwrap(),
                true,
                false,
                &[],
                &resolve(&stale, Action::Replace),
                &NEVER_CANCELLED,
            )
            .unwrap();
        assert!(result.errors.is_empty());
        assert!(!a.join("x.txt").exists());
        assert_eq!(fs::read_to_string(b.join("x.txt")).unwrap(), "source");
        op.undo().unwrap();
        assert_eq!(
            fs::read_to_string(b.join("x.txt")).unwrap(),
            "changed since prompt"
        );
        assert_eq!(fs::read_to_string(a.join("x.txt")).unwrap(), "source");
    }
    #[test]
    fn merging_folders_preserves_skipped_files_and_keep_both_keeps_extension() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        let b = t.path().join("b");
        fs::create_dir_all(a.join("folder")).unwrap();
        fs::create_dir_all(b.join("folder")).unwrap();
        fs::write(a.join("folder/x.txt"), "source").unwrap();
        fs::write(a.join("folder/new.txt"), "new").unwrap();
        fs::write(b.join("folder/x.txt"), "existing").unwrap();
        let source = vec![a.join("folder").to_string_lossy().into_owned()];
        let op = Operations::default();
        let first = op
            .transfer(
                &source,
                b.to_str().unwrap(),
                true,
                false,
                &[],
                &NEVER_CANCELLED,
            )
            .unwrap();
        op.transfer_resolved(
            &source,
            b.to_str().unwrap(),
            true,
            false,
            &[],
            &resolve(&first, Action::Skip),
            &NEVER_CANCELLED,
        )
        .unwrap();
        assert!(a.join("folder/x.txt").exists());
        assert!(!a.join("folder/new.txt").exists());
        let next = op
            .transfer(
                &source,
                b.to_str().unwrap(),
                false,
                false,
                &[],
                &NEVER_CANCELLED,
            )
            .unwrap();
        op.transfer_resolved(
            &source,
            b.to_str().unwrap(),
            false,
            false,
            &[],
            &resolve(&next, Action::KeepBoth),
            &NEVER_CANCELLED,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(b.join("folder/x (2).txt")).unwrap(),
            "source"
        );
        assert_eq!(
            fs::read_to_string(b.join("folder/x.txt")).unwrap(),
            "existing"
        );
    }
}
