//! Serialize raw-driver mounts for one device. Linux block devices are held
//! with O_EXCL for the operation, preventing an OS mount from racing the driver.
use std::{
    collections::HashMap,
    fs::File,
    marker::PhantomData,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Condvar, Mutex, OnceLock, Weak},
    thread::ThreadId,
};
#[derive(Default)]
struct State {
    owner: Option<ThreadId>,
    depth: usize,
    exclusive: Option<File>,
}
#[derive(Default)]
struct Gate {
    state: Mutex<State>,
    changed: Condvar,
}
pub(super) struct DeviceGuard {
    gate: Arc<Gate>,
    _thread: PhantomData<Rc<()>>,
}
fn gates() -> &'static Mutex<HashMap<PathBuf, Weak<Gate>>> {
    static GATES: OnceLock<Mutex<HashMap<PathBuf, Weak<Gate>>>> = OnceLock::new();
    GATES.get_or_init(Default::default)
}
pub(super) fn lock(device: &str) -> Result<DeviceGuard, String> {
    let path = dunce::canonicalize(device).map_err(|_| "StaleRef: device unavailable")?;
    let gate = {
        let mut gates = gates().lock().map_err(|_| "device registry unavailable")?;
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(&path).and_then(Weak::upgrade) {
            gate
        } else {
            let gate = Arc::new(Gate::default());
            gates.insert(path.clone(), Arc::downgrade(&gate));
            gate
        }
    };
    let thread = std::thread::current().id();
    let mut state = gate.state.lock().map_err(|_| "device lock unavailable")?;
    while state.owner.is_some_and(|owner| owner != thread) {
        state = gate
            .changed
            .wait(state)
            .map_err(|_| "device lock unavailable")?;
    }
    if state.depth == 0 {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
            let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if metadata.file_type().is_block_device() {
                state.exclusive = Some(
                    std::fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_EXCL)
                        .open(&path)
                        .map_err(|_| {
                            "PermissionDenied: raw device is mounted, busy or not granted"
                        })?,
                );
            }
        }
        state.owner = Some(thread);
    }
    state.depth += 1;
    drop(state);
    Ok(DeviceGuard {
        gate,
        _thread: PhantomData,
    })
}
impl Drop for DeviceGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.gate.state.lock() {
            state.depth -= 1;
            if state.depth == 0 {
                state.owner = None;
                state.exclusive.take();
                self.gate.changed.notify_one();
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn nested_driver_calls_are_reentrant_and_other_threads_wait() {
        let image = tempfile::NamedTempFile::new().unwrap();
        let path = image.path().to_string_lossy().into_owned();
        let first = lock(&path).unwrap();
        let nested = lock(&path).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _guard = lock(&path).unwrap();
            tx.send(()).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
        drop(first);
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
        drop(nested);
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
    }
}
