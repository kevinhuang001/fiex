//! One bounded, priority-aware worker. Disk failure never recursively logs.
use super::*;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Condvar},
};
const MAX_QUEUE: usize = 2048;
static DROPPED: AtomicU64 = AtomicU64::new(0);
enum Job {
    Append(String, LogRecord),
    Clear(String, std::sync::mpsc::SyncSender<()>),
    Flush(std::sync::mpsc::SyncSender<()>),
}
struct Writer {
    queue: Mutex<VecDeque<Job>>,
    ready: Condvar,
}
fn worker() -> &'static Arc<Writer> {
    static WRITER: OnceLock<Arc<Writer>> = OnceLock::new();
    WRITER.get_or_init(|| {
        let writer = Arc::new(Writer {
            queue: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
        });
        let copy = writer.clone();
        std::thread::Builder::new()
            .name("fiex-log-writer".into())
            .spawn(move || loop {
                let job = {
                    let mut queue = copy.queue.lock().unwrap_or_else(|e| e.into_inner());
                    while queue.is_empty() {
                        queue = copy.ready.wait(queue).unwrap_or_else(|e| e.into_inner());
                    }
                    queue.pop_front().unwrap()
                };
                match job {
                    Job::Append(id, record) => {
                        persist(&id, &record);
                        prune();
                    }
                    Job::Flush(done) => {
                        let _ = done.send(());
                    }
                    Job::Clear(id, done) => {
                        if let Some((a, b)) = paths(&id) {
                            let _ = fs::remove_file(a);
                            let _ = fs::remove_file(b);
                        }
                        let _ = done.send(());
                    }
                }
            })
            .expect("start diagnostic writer");
        writer
    })
}
pub(super) fn dropped() {
    DROPPED.fetch_add(1, Ordering::Relaxed);
}
pub(super) fn append(id: &str, mut record: LogRecord) {
    let writer = worker();
    let mut queue = writer.queue.lock().unwrap_or_else(|e| e.into_inner());
    if queue.len() >= MAX_QUEUE {
        if matches!(record.level.as_str(), "warn" | "error") {
            if let Some(index) = queue.iter().position(|job| matches!(job, Job::Append(_, r) if !matches!(r.level.as_str(), "warn" | "error"))) { queue.remove(index); dropped(); }
            else { dropped(); return; }
        } else {
            dropped();
            return;
        }
    }
    let count = DROPPED.swap(0, Ordering::Relaxed);
    if count > 0 {
        record.detail = Some(format!(
            "{}\n{count} diagnostic records dropped by backpressure/rate limits",
            record.detail.unwrap_or_default()
        ));
    }
    queue.push_back(Job::Append(id.into(), record));
    writer.ready.notify_one();
}
pub(super) fn clear(id: &str) {
    let writer = worker();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    {
        let mut queue = writer.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.retain(|job| !matches!(job, Job::Append(source, _) if source == id));
        // Reserve space without allowing a flood to evict clear barriers.
        if queue.len() >= MAX_QUEUE {
            return;
        }
        queue.push_back(Job::Clear(id.into(), tx));
        writer.ready.notify_one();
    }
    let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
}
fn directory() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("fiex").join("logs-v2"))
}
fn paths(plugin_id: &str) -> Option<(PathBuf, PathBuf)> {
    use sha2::{Digest, Sha256};
    let dir = directory()?;
    let hash = hex::encode(Sha256::digest(plugin_id.as_bytes()));
    Some((
        dir.join(format!("{hash}.jsonl")),
        dir.join(format!("{hash}.previous.jsonl")),
    ))
}
pub(super) fn load_disk(plugin_id: &str) -> VecDeque<LogRecord> {
    let mut rows = VecDeque::new();
    let Some((current, previous)) = paths(plugin_id) else {
        return rows;
    };
    for path in [previous, current] {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if let Ok(record) = serde_json::from_str::<LogRecord>(line) {
                    rows.push_front(record);
                    rows.truncate(MAX_ENTRIES);
                }
            }
        }
    }
    rows
}
fn persist(plugin_id: &str, record: &LogRecord) {
    let Some((current, previous)) = paths(plugin_id) else {
        return;
    };
    let Some(parent) = current.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).is_err() {
            return;
        }
    }
    if fs::metadata(&current).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
        let _ = fs::remove_file(&previous);
        let _ = fs::rename(&current, &previous);
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(current) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if file
                .set_permissions(fs::Permissions::from_mode(0o600))
                .is_err()
            {
                return;
            }
        }
        if let Ok(json) = serde_json::to_string(record) {
            let _ = writeln!(file, "{json}");
        }
    }
}

pub(super) fn flush() {
    let writer = worker();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    {
        let mut queue = writer.queue.lock().unwrap_or_else(|e| e.into_inner());
        if queue.len() >= MAX_QUEUE {
            return;
        }
        queue.push_back(Job::Flush(tx));
        writer.ready.notify_one();
    }
    let _ = rx.recv_timeout(std::time::Duration::from_millis(750));
}
fn prune() {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    if !WRITES.fetch_add(1, Ordering::Relaxed).is_multiple_of(64) {
        return;
    }
    let Some(root) = directory() else {
        return;
    };
    let Ok(items) = fs::read_dir(root) else {
        return;
    };
    let mut files: Vec<_> = items
        .flatten()
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            (metadata.is_file() && entry.path().extension().is_some_and(|e| e == "jsonl"))
                .then_some((entry.path(), metadata.modified().ok(), metadata.len()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|v| v.2).sum();
    let mut count = files.len();
    files.sort_by_key(|v| v.1);
    for (path, _, bytes) in files {
        if total <= 64 * 1024 * 1024 && count <= 256 {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(bytes);
            count -= 1;
        }
    }
}
