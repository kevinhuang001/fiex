use super::*;

pub(super) fn execute(
    steps: &[Step],
    cancel: &AtomicBool,
    progress: &impl Fn(u64, u64, Option<&str>),
) -> Result<(), String> {
    let total = steps.len() as u64;
    // Stage all overwrite backups before touching any destination. If the
    // backup itself fails, no partial transfer has taken place.
    let mut undo = UndoLog::prepare(steps, cancel)?;
    for (index, step) in steps.iter().enumerate() {
        let attempt = (|| -> Result<(), String> {
            check_cancelled(cancel)?;
            if !same_version(&step.source.stat()?, &step.source_version) {
                return Err(format!("Conflict: source changed: {}", step.source.path));
            }
            let latest = optional_stat(&step.target)?;
            if !matches!((&step.target_version, &latest), (None, None))
                && !matches!((&step.target_version, &latest), (Some(expected), Some(actual)) if same_version(expected, actual))
            {
                return Err(format!(
                    "Conflict: destination changed: {}",
                    step.target.path
                ));
            }
            if step.directory {
                if step.existing {
                    if step.target.stat()?["isDir"] != true {
                        return Err("Conflict: destination is no longer a directory".into());
                    }
                } else {
                    step.target.require(FsOperation::Mkdir, true)?;
                    step.target.mkdir()?;
                }
            } else {
                step.source.require(FsOperation::Read, false)?;
                if step.existing && step.target.is_dir()? {
                    return Err("Conflict: destination is now a directory".into());
                }
                let create = !step.existing || optional_stat(&step.target)?.is_none();
                step.target.require(FsOperation::Write, create)?;
                let mut reader = step.source.adapter()?.open_read()?;
                let mut writer = step.target.adapter()?.begin_write(create)?;
                let mut offset = 0u64;
                loop {
                    check_cancelled(cancel)?;
                    let (bytes, eof) = reader.read_at(offset, CHUNK)?;
                    if bytes.is_empty() && !eof {
                        return Err("source stream ended unexpectedly".into());
                    }
                    if !bytes.is_empty() {
                        writer.write(&bytes)?;
                    }
                    offset = offset
                        .checked_add(bytes.len() as u64)
                        .ok_or("transfer offset overflow")?;
                    if eof {
                        break;
                    }
                }
                if offset != reader.size() {
                    return Err("source changed during transfer".into());
                }
                if !same_version(&step.source.stat()?, &step.source_version) {
                    return Err("Conflict: source changed during transfer".into());
                }
                writer.commit()?;
            }
            // Only successful mutations belong to the undo log. A rejected
            // create must never cause rollback to delete somebody else's file.
            if !step.directory || !step.existing {
                undo.record(index, step.target.stat().ok());
            }
            Ok(())
        })();
        if let Err(error) = attempt {
            let failures = undo.rollback(steps);
            if failures.is_empty() {
                return Err(error);
            }
            let recovery = match undo.preserve(steps) {
                Ok(path) => format!("recovery retained at {}", path.display()),
                Err(error) => format!("recovery metadata failed: {error}"),
            };
            return Err(format!(
                "{error}; rollback incomplete: {}; {recovery}",
                failures.join("; ")
            ));
        }
        progress(index as u64 + 1, total, Some(&step.source.path));
    }
    Ok(())
}
