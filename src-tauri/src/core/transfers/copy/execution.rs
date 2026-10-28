use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute(
    sources: &[String],
    destination: &str,
    moving: bool,
    keep_both: bool,
    ignores: &[String],
    resolutions: &[Resolution],
    cancel: &AtomicBool,
    progress: &(dyn Fn(u64, u64, Option<&str>) + Sync),
) -> Result<(OperationResult, Vec<Undo>), String> {
    check_cancelled(cancel)?;
    let dest_dir = dunce::canonicalize(destination).map_err(|e| e.to_string())?;
    if !dest_dir.is_dir() {
        return Err(crate::i18n::t("目标不是文件夹").into());
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
            return Err(crate::i18n::t("不能把文件夹复制或移动到其自身内部").into());
        }
        let name = src
            .file_name()
            .ok_or(crate::i18n::t("不能操作根目录"))?
            .to_os_string();
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
    let total = plan.tasks.iter().map(|task| task.files).sum::<u64>();
    let progress = Progress::new(total, progress);
    if !moving && plan.tasks.iter().all(|task| !task.replace) {
        let ignore = &plan.ignore;
        let tasks = std::mem::take(&mut plan.tasks);
        let workers = std::thread::available_parallelism()
            .map(|value| value.get().min(8))
            .unwrap_or(4);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .map_err(|e| e.to_string())?;
        let outcomes = pool.install(|| {
            use rayon::prelude::*;
            tasks
                .into_par_iter()
                .map(|task| {
                    let result = if cancel.load(Ordering::Relaxed) {
                        Err(CANCELLED.into())
                    } else if exists(&task.dest) {
                        Err(crate::i18n::t("目标已改变，请重试").into())
                    } else {
                        install_copy(&task.src, &task.dest, ignore, cancel, Some(&progress))
                    };
                    (task, result)
                })
                .collect::<Vec<_>>()
        });
        let mut cancelled = false;
        for (task, outcome) in outcomes {
            match outcome {
                Ok(skipped) => {
                    result.skipped += skipped;
                    result.paths.push(task.dest.to_string_lossy().into_owned());
                    undo.push(Undo::Created(task.dest));
                }
                Err(error) => {
                    if cancel.load(Ordering::Relaxed) || error == CANCELLED {
                        cancelled = true;
                    } else {
                        result
                            .errors
                            .push(format!("{}: {error}", task.src.display()));
                    }
                }
            }
        }
        if cancelled {
            return Err(CANCELLED.into());
        }
        progress.finish();
        return Ok((result, undo));
    }
    for task in plan.tasks {
        // Cancellation stops at a file boundary; moved files are not rolled back.
        check_cancelled(cancel)?;
        let Task {
            src,
            dest,
            replace,
            files,
        } = task;
        // Re-check immediately before mutation: a new target must get a fresh dialog.
        if exists(&dest) && !replace {
            result.errors.push(crate::i18n::format(
                "{0}: 目标已改变，请重试",
                &[("0", format!("{}", dest.display()))],
            ));
            continue;
        }
        let backup = if replace {
            let storage = tempfile::Builder::new()
                .prefix(".fiex-undo-")
                .tempdir_in(dest.parent().ok_or(crate::i18n::t("目标路径无效"))?)
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
            install_copy(&src, &dest, &plan.ignore, cancel, Some(&progress))
        };
        match operation {
            Ok(skipped) => {
                result.skipped += skipped;
                result.paths.push(dest.to_string_lossy().into_owned());
                if moving {
                    progress.record_files(&dest, files);
                }
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
                            result.errors.push(crate::i18n::format(
                                "目标已改变，原文件保存在 {0}",
                                &[("0", format!("{}", saved.display()))],
                            ));
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
    progress.finish();
    if moving {
        for src in plan.merged.into_iter().rev() {
            let _ = fs::remove_dir(src);
        }
    }
    Ok((result, undo))
}
