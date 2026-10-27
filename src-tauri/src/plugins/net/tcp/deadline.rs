//! A total broker deadline covers DNS, connect, lock contention and partial IO.
use serde_json::Value;
use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Mutex, MutexGuard, TryLockError,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub(super) struct Deadline(Instant);
impl Deadline {
    pub fn new(params: &Value, default: Duration) -> Result<Self, String> {
        let duration = if let Some(unix_ms) = params["deadlineMs"].as_u64() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_millis();
            if unix_ms as u128 <= now {
                return Err("TCP deadline exceeded".into());
            }
            Duration::from_millis((unix_ms as u128 - now).min(120_000) as u64)
        } else {
            default
        };
        Ok(Self(Instant::now() + duration))
    }
    pub fn remaining(&self) -> Result<Duration, String> {
        self.0
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or_else(|| "TCP deadline exceeded".into())
    }
    pub fn lock<'a, T>(&self, mutex: &'a Mutex<T>) -> Result<MutexGuard<'a, T>, String> {
        loop {
            match mutex.try_lock() {
                Ok(guard) => {
                    self.remaining()?;
                    return Ok(guard);
                }
                Err(TryLockError::Poisoned(_)) => return Err("TCP connection unavailable".into()),
                Err(TryLockError::WouldBlock) => {
                    std::thread::park_timeout(self.remaining()?.min(Duration::from_millis(2)))
                }
            }
        }
    }
    pub fn resolve(&self, host: String, port: u16) -> Result<Vec<SocketAddr>, String> {
        static ACTIVE: AtomicUsize = AtomicUsize::new(0);
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                ACTIVE.fetch_sub(1, Ordering::AcqRel);
            }
        }
        #[allow(deprecated)]
        ACTIVE
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < 8).then_some(active + 1)
            })
            .map_err(|_| "TCP DNS resolver busy")?;
        let guard = Guard;
        let (send, receive) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("fiex-dns".into())
            .spawn(move || {
                let _guard = guard;
                let result = (host.as_str(), port)
                    .to_socket_addrs()
                    .map(|addresses| addresses.collect());
                let _ = send.send(result);
            })
            .map_err(|error| error.to_string())?;
        receive
            .recv_timeout(self.remaining()?)
            .map_err(|_| "TCP deadline exceeded during DNS")?
            .map_err(|error| error.to_string())
    }
}
pub(super) fn io_error(error: io::Error) -> String {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        "TCP deadline exceeded".into()
    } else {
        error.to_string()
    }
}
