//! Temporary rollback journal for multi-source transfers. Existing file
//! versions are spooled before the first destination commit, never buffered
//! into an unbounded Vec. All cleanup stays within the destination adapters.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};

pub(super) struct UndoLog {
    backups: Vec<Option<tempfile::NamedTempFile>>,
    affected: Vec<(usize, Option<serde_json::Value>)>,
}
impl UndoLog {
    pub fn prepare(steps: &[Step], cancel: &AtomicBool) -> Result<Self, String> {
        let mut backups = Vec::with_capacity(steps.len());
        for step in steps {
            check_cancelled(cancel)?;
            if !step.existing || step.directory {
                backups.push(None);
                continue;
            }
            step.target.require(FsOperation::Read, false)?;
            let mut reader = step.target.adapter()?.open_read()?;
            let mut copy = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
            let mut offset = 0u64;
            loop {
                check_cancelled(cancel)?;
                let (bytes, eof) = reader.read_at(offset, CHUNK)?;
                if bytes.is_empty() && !eof {
                    return Err("backup stream stalled".into());
                }
                copy.write_all(&bytes).map_err(|e| e.to_string())?;
                offset = offset
                    .checked_add(bytes.len() as u64)
                    .ok_or("backup size overflow")?;
                if eof {
                    break;
                }
            }
            if offset != reader.size()
                || !same_version(
                    &step.target.stat()?,
                    step.target_version
                        .as_ref()
                        .ok_or("missing target version")?,
                )
            {
                return Err("Conflict: destination changed before transfer".into());
            }
            copy.as_file().sync_all().map_err(|e| e.to_string())?;
            backups.push(Some(copy));
        }
        Ok(Self {
            backups,
            affected: Vec::new(),
        })
    }

    pub fn record(&mut self, index: usize, version: Option<serde_json::Value>) {
        self.affected.push((index, version));
    }

    pub fn rollback(&mut self, steps: &[Step]) -> Vec<String> {
        let mut errors = Vec::new();
        for (index, committed) in self.affected.clone().into_iter().rev() {
            let step = &steps[index];
            if step.existing && step.directory {
                continue;
            }
            // Rollback must not overwrite a concurrent editor's later version
            // or recursively remove children created outside this transfer.
            let current = optional_stat(&step.target);
            if step.directory {
                if let Ok(Some(_)) = &current {
                    match step.target.list() {
                        Ok(items) if items.as_array().is_some_and(Vec::is_empty) => {}
                        _ => {
                            errors.push(format!(
                                "rollback {}: directory is not empty",
                                step.target.path
                            ));
                            continue;
                        }
                    }
                }
            } else if !matches!((&committed, &current), (Some(expected), Ok(Some(actual))) if same_version(expected, actual))
            {
                errors.push(format!(
                    "rollback {}: destination changed or unavailable",
                    step.target.path
                ));
                continue;
            }
            let result = if step.existing {
                self.restore(index, &step.target)
            } else {
                match step.target.remove() {
                    Ok(()) => Ok(()),
                    Err(error) if is_missing(&error) => Ok(()),
                    Err(error) => Err(error),
                }
            };
            if let Err(error) = result {
                errors.push(format!("rollback {}: {error}", step.target.path));
            }
        }
        errors
    }

    /// Retain original bytes when an offline target prevents rollback. No
    /// saved scope is replayed automatically; recovery needs fresh selection.
    pub fn preserve(&mut self, steps: &[Step]) -> Result<std::path::PathBuf, String> {
        let base = dirs::data_local_dir()
            .ok_or("recovery directory unavailable")?
            .join("fiex")
            .join("transfer-recovery");
        std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let directory = tempfile::Builder::new()
            .prefix("transfer-")
            .tempdir_in(base)
            .map_err(|e| e.to_string())?
            .keep();
        let mut records = Vec::new();
        for (index, backup) in self.backups.iter_mut().enumerate() {
            let Some(backup) = backup.take() else {
                continue;
            };
            let destination = directory.join(format!("{index}.backup"));
            // A cross-device rename may fail. Keep the original private file
            // rather than discard the only remaining pre-transfer version.
            let (file, path) = match backup.persist_noclobber(&destination) {
                Ok(file) => (file, destination),
                Err(error) => error.file.keep().map_err(|e| e.to_string())?,
            };
            file.sync_all().map_err(|e| e.to_string())?;
            records.push(serde_json::json!({"target":steps[index].target.path,
                "ref":steps[index].target.reference,"backup":path,
                "version":steps[index].target_version}));
        }
        let mut manifest =
            tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(
            manifest.as_file_mut(),
            &serde_json::json!({"backups":records}),
        )
        .map_err(|e| e.to_string())?;
        manifest.as_file().sync_all().map_err(|e| e.to_string())?;
        manifest
            .persist(directory.join("recovery.json"))
            .map_err(|e| e.to_string())?;
        Ok(directory)
    }

    fn restore(&mut self, index: usize, target: &ResourceBroker) -> Result<(), String> {
        let backup = self.backups[index]
            .as_mut()
            .ok_or("missing rollback backup")?;
        backup
            .as_file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        target.require(FsOperation::Write, false)?;
        let mut writer = target.adapter()?.begin_write(false)?;
        let mut buffer = vec![0u8; CHUNK];
        loop {
            let read = backup.read(&mut buffer).map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            writer.write(&buffer[..read])?;
        }
        writer.commit()?;
        Ok(())
    }
}
