//! Filesystem primitives: create and rename entries, move items to the Trash
//! and copy trees.

use super::{check_cancelled, exists, OperationResult};
use crate::{core::files::validate_name, core::ignores::IgnoreSet};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
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
    if exists(&dest) {
        return Err(crate::i18n::t("同名项目已存在，请更换名称").into());
    }
    fs::rename(src, &dest).map_err(|e| e.to_string())?;
    Ok(Some(dest))
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

struct CopyFile {
    src: PathBuf,
    dest: PathBuf,
    permissions: fs::Permissions,
}

#[derive(Default)]
struct CopyPlan {
    dirs: Vec<(PathBuf, PathBuf)>,
    files: Vec<CopyFile>,
    skipped: usize,
}

fn collect_copy(
    src: &Path,
    dest: &Path,
    ignore: &IgnoreSet,
    relative: &Path,
    plan: &mut CopyPlan,
) -> Result<(), String> {
    if ignore.is_ignored(relative) {
        plan.skipped += 1;
        return Ok(());
    }
    let meta = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        return Err(crate::i18n::t("为避免复制到意外位置，请单独处理符号链接").into());
    }
    if meta.is_dir() {
        plan.dirs.push((src.into(), dest.into()));
        for item in fs::read_dir(src).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            collect_copy(
                &item.path(),
                &dest.join(item.file_name()),
                ignore,
                &relative.join(item.file_name()),
                plan,
            )?;
        }
    } else if meta.is_file() {
        plan.files.push(CopyFile {
            src: src.into(),
            dest: dest.into(),
            permissions: meta.permissions(),
        });
    } else {
        return Err(crate::i18n::t("不能复制设备或特殊文件").into());
    }
    Ok(())
}

fn fast_copy_file(src: &Path, dest: &Path, cancel: &AtomicBool) -> Result<(), String> {
    check_cancelled(cancel)?;
    // CoW clone is effectively instant on APFS, Btrfs, XFS and ReFS.
    if reflink_copy::reflink(src, dest).is_ok() {
        return Ok(());
    }
    // A failed clone can leave its temporary destination behind.
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    let mut input = fs::File::open(src).map_err(|e| e.to_string())?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|e| e.to_string())?;
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    loop {
        check_cancelled(cancel)?;
        let read = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn copy_tree_with_progress(
    src: &Path,
    dest: &Path,
    ignore: &IgnoreSet,
    relative: &Path,
    cancel: &AtomicBool,
    progress: Option<&(dyn Fn(&Path) + Sync)>,
) -> Result<usize, String> {
    check_cancelled(cancel)?;
    let mut plan = CopyPlan::default();
    collect_copy(src, dest, ignore, relative, &mut plan)?;
    for (_, dir) in &plan.dirs {
        fs::create_dir(dir).map_err(|e| e.to_string())?;
    }
    let report = |file: &Path| {
        if let Some(progress) = progress {
            progress(file);
        }
    };
    if plan.files.len() <= 1 {
        for file in &plan.files {
            fast_copy_file(&file.src, &file.dest, cancel)?;
            fs::set_permissions(&file.dest, file.permissions.clone()).map_err(|e| e.to_string())?;
            report(&file.dest);
        }
        return Ok(plan.skipped);
    }
    let copy_files = || {
        use rayon::prelude::*;
        plan.files.par_iter().try_for_each(|file| {
            fast_copy_file(&file.src, &file.dest, cancel)?;
            fs::set_permissions(&file.dest, file.permissions.clone()).map_err(|e| e.to_string())?;
            report(&file.dest);
            Ok::<(), String>(())
        })
    };
    if rayon::current_thread_index().is_some() {
        copy_files()?;
    } else {
        let workers = std::thread::available_parallelism()
            .map(|value| value.get().min(8))
            .unwrap_or(4);
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .map_err(|e| e.to_string())?
            .install(copy_files)?;
    }
    Ok(plan.skipped)
}

pub fn recycle(paths: &[String]) -> Result<OperationResult, String> {
    for path in paths {
        crate::plugins::mounts::reject_local(path)?;
    }
    let mut result = OperationResult::default();
    for path in paths {
        let before = crate::core::volumes::trash_names();
        match trash::delete(path) {
            Ok(()) => {
                crate::core::volumes::record_trashed(path, &before);
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
    use crate::core::operations::CANCELLED;
    use std::sync::atomic::Ordering;

    #[test]
    fn parallel_copy_preserves_every_file() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("tree");
        let dest = t.path().join("copy");
        fs::create_dir_all(src.join("nested")).unwrap();
        for index in 0..64 {
            let path = if index % 2 == 0 {
                src.join(format!("file-{index}.bin"))
            } else {
                src.join("nested").join(format!("file-{index}.bin"))
            };
            fs::write(path, vec![index as u8; 128 * 1024]).unwrap();
        }
        copy_tree_with_progress(
            &src,
            &dest,
            &IgnoreSet::new(&[]),
            Path::new("tree"),
            &AtomicBool::new(false),
            None,
        )
        .unwrap();
        for index in 0..64 {
            let path = if index % 2 == 0 {
                dest.join(format!("file-{index}.bin"))
            } else {
                dest.join("nested").join(format!("file-{index}.bin"))
            };
            assert_eq!(fs::read(path).unwrap(), vec![index as u8; 128 * 1024]);
        }
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
}
