//! Stream staging for imported Applications assets.
use super::applications;
use std::{
    io::{self, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct ApplicationStage {
    pub(super) file: Arc<Mutex<Option<tempfile::NamedTempFile>>>,
    pub(super) uri: String,
    pub(super) committed: Arc<AtomicBool>,
}

impl Write for ApplicationStage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut staged = self
            .file
            .lock()
            .map_err(|_| io::Error::other("write stream unavailable"))?;
        staged
            .as_mut()
            .ok_or_else(|| io::Error::other("write stream closed"))?
            .write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut staged = self
            .file
            .lock()
            .map_err(|_| io::Error::other("write stream unavailable"))?;
        staged
            .as_mut()
            .ok_or_else(|| io::Error::other("write stream closed"))?
            .flush()
    }
}

impl Drop for ApplicationStage {
    fn drop(&mut self) {
        if !self.committed.load(Ordering::Acquire) {
            applications::cancel_reservation(&self.uri);
        }
    }
}
