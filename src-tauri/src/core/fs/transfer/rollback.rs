//! Temporary rollback journal for multi-source transfers. Existing file
//! versions are spooled before the first destination commit, never buffered
//! into an unbounded Vec. All cleanup stays within the destination adapters.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};

pub(super) struct UndoLog {
    backups: Vec<Option<tempfile::NamedTempFile>>,
    affected: Vec<usize>,
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
                offset = offset.checked_add(bytes.len() as u64)
                    .ok_or("backup size overflow")?;
                if eof { break; }
            }
            if offset != reader.size()
                || !same_version(&step.target.stat()?, step.target_version.as_ref()
                    .ok_or("missing target version")?)
            {
                return Err("Conflict: destination changed before transfer".into());
            }
            copy.as_file().sync_all().map_err(|e| e.to_string())?;
            backups.push(Some(copy));
        }
        Ok(Self { backups, affected: Vec::new() })
    }

    pub fn record(&mut self, index: usize) {
        self.affected.push(index);
    }

    pub fn rollback(&mut self, steps: &[Step]) -> Vec<String> {
        let mut errors = Vec::new();
        for index in self.affected.clone().into_iter().rev() {
            let step = &steps[index];
            if step.existing && step.directory { continue; }
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

    fn restore(&mut self, index: usize, target: &ResourceBroker) -> Result<(), String> {
        let backup = self.backups[index].as_mut().ok_or("missing rollback backup")?;
        backup.as_file_mut().seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        target.require(FsOperation::Write, false)?;
        let mut writer = target.adapter()?.begin_write(false)?;
        let mut buffer = vec![0u8; CHUNK];
        loop {
            let read = backup.read(&mut buffer).map_err(|e| e.to_string())?;
            if read == 0 { break; }
            writer.write(&buffer[..read])?;
        }
        writer.commit()?;
        Ok(())
    }
}
