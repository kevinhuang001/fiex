//! Transfers through raw filesystem drivers use the same conflict choices as
//! local files. All conflicts are collected before writes begin.
use crate::{
    files::{self, Entry},
    ignores::IgnoreSet,
    operations::{check_cancelled, OperationResult, CANCELLED},
    transfers::{Action, Conflict, FileVersion, Resolution},
    usbfs,
};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
fn join(dir: &str, name: &str) -> String {
    format!(
        "{}{}{}",
        dir.trim_end_matches(['/', '\\']),
        if dir.contains('\\') { "\\" } else { "/" },
        name
    )
}
fn list(path: &str) -> Result<Vec<Entry>, String> {
    if let Some(r) = usbfs::route(path) {
        usbfs::listing(&r)
    } else {
        files::list(path, true, &[]).map(|l| l.entries)
    }
}
fn info(path: &str) -> Result<Entry, String> {
    if let Some(r) = usbfs::route(path) {
        usbfs::entry(&r)
    } else {
        files::entry(Path::new(path))
    }
}
fn parent(path: &str) -> Result<&str, String> {
    path.rsplit_once(['/', '\\'])
        .map(|(p, _)| p)
        .ok_or_else(|| "无效路径".into())
}
fn read(path: &str) -> Result<Vec<u8>, String> {
    if let Some(r) = usbfs::route(path) {
        usbfs::read_path(&r)
    } else {
        fs::read(path).map_err(|e| e.to_string())
    }
}
fn writable(path: &str) -> Result<(), String> {
    if usbfs::route(path).is_some_and(|r| {
        matches!(
            r.kind,
            usbfs::FsKind::Apfs | usbfs::FsKind::Hfs | usbfs::FsKind::Btrfs
        )
    }) {
        return Err("此文件系统只读".into());
    }
    Ok(())
}
fn create(path: &str, folder: bool) -> Result<(), String> {
    if let Some(r) = usbfs::route(parent(path)?) {
        usbfs::create(
            &r,
            Path::new(path)
                .file_name()
                .ok_or("无效名称")?
                .to_string_lossy()
                .as_ref(),
            folder,
        )
    } else if folder {
        fs::create_dir(path).map_err(|e| e.to_string())
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
fn write(path: &str, data: &[u8]) -> Result<(), String> {
    if let Some(r) = usbfs::route(path) {
        usbfs::write_path(&r, data)
    } else {
        fs::write(path, data).map_err(|e| e.to_string())
    }
}
fn remove(path: &str) -> Result<(), String> {
    if info(path)?.is_dir {
        for child in list(path)? {
            remove(&child.path)?;
        }
    }
    if let Some(r) = usbfs::route(path) {
        usbfs::remove(&r)
    } else if info(path)?.is_dir {
        fs::remove_dir(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}
fn rename(path: &str, name: &str) -> Result<String, String> {
    let to = join(parent(path)?, name);
    if let Some(r) = usbfs::route(path) {
        usbfs::rename(&r, name)?;
    } else {
        fs::rename(path, &to).map_err(|e| e.to_string())?;
    }
    Ok(to)
}
fn version(e: &Entry) -> FileVersion {
    FileVersion {
        path: e.path.clone(),
        size: e.size,
        modified: e.modified,
        is_dir: e.is_dir,
    }
}
struct Task {
    source: Entry,
    target: String,
    existing: bool,
}
struct Plan<'a> {
    tasks: Vec<Task>,
    conflicts: Vec<Conflict>,
    dirs: Vec<(String, String)>,
    merges: Vec<String>,
    skipped: usize,
    ignores: IgnoreSet,
    resolutions: &'a [Resolution],
    keep_both: bool,
    moving: bool,
    cancel: &'a AtomicBool,
}
impl Plan<'_> {
    fn walk(
        &mut self,
        source: Entry,
        target: String,
        existing: Option<Entry>,
        relative: String,
    ) -> Result<(), String> {
        check_cancelled(self.cancel)?;
        if self.ignores.is_ignored(Path::new(&relative)) {
            self.skipped += 1;
            return Ok(());
        }
        if source.is_symlink {
            return Err("暂不支持复制符号链接".into());
        }
        if self.moving {
            writable(&source.path)?;
        }
        if source.path == target && self.moving {
            return Ok(());
        }
        let mut target = target;
        let mut replace = false;
        if let Some(ref old) = existing {
            if source.is_dir && old.is_dir && source.path != target && !self.keep_both {
                self.merges.push(source.path.clone());
                let children = list(&target)?;
                for child in list(&source.path)? {
                    let found = children.iter().find(|e| e.name == child.name).cloned();
                    let to = join(&target, &child.name);
                    let rel = join(&relative, &child.name);
                    self.walk(child, to, found, rel)?;
                }
                return Ok(());
            }
            let key = format!(
                "{}\0{}\0{}\0{}\0{}\0{}",
                source.path, old.path, source.modified, old.modified, source.size, old.size
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
                        source: version(&source),
                        destination: version(old),
                    });
                    return Ok(());
                }
                Some(Action::Skip) => {
                    self.skipped += 1;
                    return Ok(());
                }
                Some(Action::Replace) => {
                    if source.path == target {
                        return Err("不能替换自身，请选择保留两者".into());
                    }
                    replace = true;
                }
                Some(Action::KeepBoth) => {
                    let parent = parent(&target)?;
                    let siblings = list(parent)?;
                    let p = Path::new(&source.name);
                    let stem = if source.is_dir {
                        p.file_name()
                    } else {
                        p.file_stem()
                    }
                    .unwrap_or_default()
                    .to_string_lossy();
                    let extension = if source.is_dir {
                        String::new()
                    } else {
                        p.extension()
                            .map(|e| format!(".{}", e.to_string_lossy()))
                            .unwrap_or_default()
                    };
                    for n in 2.. {
                        let name = format!("{stem} ({n}){extension}");
                        if !siblings.iter().any(|e| e.name == name) {
                            target = join(parent, &name);
                            break;
                        }
                    }
                }
            }
        }
        if source.is_dir && !replace {
            self.dirs.push((source.path.clone(), target.clone()));
            for child in list(&source.path)? {
                let to = join(&target, &child.name);
                let rel = join(&relative, &child.name);
                self.walk(child, to, None, rel)?;
            }
        } else {
            self.tasks.push(Task {
                source,
                target,
                existing: replace,
            });
        }
        Ok(())
    }
}
fn copy_node(source: &Entry, target: &str, cancel: &AtomicBool) -> Result<(), String> {
    check_cancelled(cancel)?;
    if source.is_dir {
        create(target, true)?;
        for child in list(&source.path)? {
            copy_node(&child, &join(target, &child.name), cancel)?;
        }
    } else {
        let data = read(&source.path)?;
        check_cancelled(cancel)?;
        create(target, false)?;
        write(target, &data)?;
        if read(target)? != data {
            return Err("写入后内容校验失败，源文件已保留".into());
        }
    }
    Ok(())
}
pub fn transfer(
    sources: &[String],
    destination: &str,
    moving: bool,
    keep_both: bool,
    ignores: &[String],
    resolutions: &[Resolution],
    cancel: &AtomicBool,
) -> Result<OperationResult, String> {
    check_cancelled(cancel)?;
    writable(destination)?;
    let children = list(destination)?;
    let mut plan = Plan {
        tasks: vec![],
        conflicts: vec![],
        dirs: vec![],
        merges: vec![],
        skipped: 0,
        ignores: IgnoreSet::new(ignores),
        resolutions,
        keep_both,
        moving,
        cancel,
    };
    for path in sources {
        check_cancelled(cancel)?;
        if destination == path
            || destination.starts_with(&format!("{path}/"))
            || destination.starts_with(&format!("{path}\\"))
        {
            return Err("不能移动到自身内部".into());
        }
        let source = info(path)?;
        let existing = children.iter().find(|e| e.name == source.name).cloned();
        plan.walk(
            source.clone(),
            join(destination, &source.name),
            existing,
            source.name,
        )?;
    }
    let mut result = OperationResult {
        conflicts: plan.conflicts,
        skipped: plan.skipped,
        ..Default::default()
    };
    if !result.conflicts.is_empty() {
        return Ok(result);
    }
    for (_, to) in &plan.dirs {
        check_cancelled(cancel)?;
        create(to, true)?;
    }
    for (index, task) in plan.tasks.into_iter().enumerate() {
        check_cancelled(cancel)?;
        let backup = if task.existing {
            let name = format!(
                ".fiex-replaced-{}-{index}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            Some(rename(&task.target, &name)?)
        } else {
            None
        };
        let result_copy = copy_node(&task.source, &task.target, cancel);
        match result_copy {
            Ok(()) => {
                if moving {
                    remove(&task.source.path)?;
                }
                if let Some(path) = backup {
                    remove(&path)?;
                }
                result.paths.push(task.target);
            }
            Err(error) => {
                if info(&task.target).is_ok() {
                    remove(&task.target)?;
                }
                if let Some(path) = backup {
                    rename(
                        &path,
                        Path::new(&task.target)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .as_ref(),
                    )?;
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err(CANCELLED.into());
                }
                result.errors.push(error);
            }
        }
    }
    if moving {
        for (src, _) in plan
            .dirs
            .into_iter()
            .rev()
            .chain(plan.merges.into_iter().map(|src| (src, String::new())))
        {
            if list(&src).is_ok_and(|v| v.is_empty()) {
                remove(&src)?;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfers::NEVER_CANCELLED;
    #[test]
    fn image_transfers_conflicts_and_moves_verify_actual_bytes() {
        use fs_ntfs::block_io::{BlockIo, PathIo};
        let t = tempfile::tempdir().unwrap();
        let image = t.path().join("ntfs.img");
        fs::File::create(&image)
            .unwrap()
            .set_len(64 * 1024 * 1024)
            .unwrap();
        let mut io = PathIo::open_rw(&image).unwrap();
        fs_ntfs::mkfs::format_filesystem(
            &mut io,
            64 * 1024 * 1024,
            4096,
            4096,
            Some("FIEX"),
            Some(1),
        )
        .unwrap();
        io.sync().unwrap();
        drop(io);
        let root = usbfs::virtual_root(image.to_str().unwrap(), usbfs::FsKind::Ntfs);
        let local = t.path().join("source.txt");
        fs::write(&local, "first").unwrap();
        let paths = vec![local.to_string_lossy().into_owned()];
        let result = transfer(&paths, &root, false, false, &[], &[], &NEVER_CANCELLED).unwrap();
        assert!(result.errors.is_empty());
        let in_image = join(&root, "source.txt");
        assert_eq!(read(&in_image).unwrap(), b"first");
        fs::write(&local, "updated").unwrap();
        let conflict = transfer(&paths, &root, false, false, &[], &[], &NEVER_CANCELLED).unwrap();
        assert_eq!(conflict.conflicts.len(), 1);
        assert_eq!(read(&in_image).unwrap(), b"first");
        let choices = vec![Resolution {
            key: conflict.conflicts[0].key.clone(),
            action: Action::Replace,
        }];
        transfer(&paths, &root, false, false, &[], &choices, &NEVER_CANCELLED).unwrap();
        assert_eq!(read(&in_image).unwrap(), b"updated");
        let export = t.path().join("export");
        fs::create_dir(&export).unwrap();
        transfer(
            std::slice::from_ref(&in_image),
            export.to_str().unwrap(),
            true,
            false,
            &[],
            &[],
            &NEVER_CANCELLED,
        )
        .unwrap();
        assert_eq!(fs::read(export.join("source.txt")).unwrap(), b"updated");
        assert!(info(&in_image).is_err());
        assert!(local.exists());
    }
    #[test]
    fn readonly_driver_destinations_are_rejected() {
        for kind in [
            usbfs::FsKind::Apfs,
            usbfs::FsKind::Hfs,
            usbfs::FsKind::Btrfs,
        ] {
            let path = usbfs::virtual_root("/tmp/nonexistent-ci.img", kind);
            assert!(writable(&path).is_err());
        }
    }
}
