//! Host-owned aria2 streams used by the SDK `ctx.aria2()` API.
//!
//! Download configuration, engine execution and stream lifecycle are split
//! into focused modules so each boundary can be tested independently.
mod download;
mod options;
mod stream;

pub(crate) use stream::{close, open, read};

use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

struct Shared {
    done: bool,
    error: Option<String>,
    total: u64,
}

struct Stream {
    owner: String,
    _directory: tempfile::TempDir,
    payload: PathBuf,
    file: Option<fs::File>,
    offset: u64,
    // Multi-segment downloads may write later pieces first. File length is
    // NOT a contiguous-byte watermark, so read them only once complete.
    segmented: bool,
    started: Instant,
    shared: Arc<Mutex<Shared>>,
    cancel: tokio::sync::watch::Sender<bool>,
}

fn registry() -> &'static Mutex<HashMap<String, Arc<Mutex<Stream>>>> {
    static STREAMS: OnceLock<Mutex<HashMap<String, Arc<Mutex<Stream>>>>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("aria2-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}
