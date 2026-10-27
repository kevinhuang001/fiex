use super::*;

pub(super) enum UploadResult {
    Pending,
    Ready(Arc<Mutex<reqwest::blocking::Response>>),
    Failed(String),
}

/// One streamed upload. The worker publishes exactly one result; readers of
/// `host.net.http.response`/`read`/`close` observe it.
pub(super) struct Upload {
    pub(super) pipe: Arc<BodyPipe>,
    pub(super) timeout: Duration,
    pub(super) result: Mutex<UploadResult>,
    pub(super) changed: Condvar,
}

impl Upload {
    pub(super) fn new(content_length: u64, timeout: Duration) -> Self {
        Self {
            pipe: Arc::new(BodyPipe::new(content_length, Instant::now() + timeout)),
            timeout,
            result: Mutex::new(UploadResult::Pending),
            changed: Condvar::new(),
        }
    }

    pub(super) fn publish(&self, result: Result<reqwest::blocking::Response, String>) {
        // Keep a single lock order (pipe, then result) with `abort`.
        self.pipe.close();
        {
            let mut state = self
                .result
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if matches!(*state, UploadResult::Pending) {
                *state = match result {
                    Ok(response) => UploadResult::Ready(Arc::new(Mutex::new(response))),
                    Err(error) => UploadResult::Failed(error),
                };
            }
        }
        self.changed.notify_all();
    }

    pub(super) fn abort(&self) {
        self.pipe.abort("HTTP request cancelled");
        {
            let mut state = self
                .result
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if matches!(*state, UploadResult::Pending) {
                *state = UploadResult::Failed("HTTP request cancelled".into());
            }
        }
        self.changed.notify_all();
    }

    /// Wait for the worker to publish the response head. The deadline keeps a
    /// plugin from blocking forever if the worker cannot make progress.
    pub(super) fn wait(&self) -> Result<Arc<Mutex<reqwest::blocking::Response>>, String> {
        let deadline = Instant::now() + self.timeout + Duration::from_secs(5);
        let mut state = self
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        loop {
            match &*state {
                UploadResult::Ready(response) => return Ok(response.clone()),
                UploadResult::Failed(error) => return Err(error.clone()),
                UploadResult::Pending => {}
            }
            let now = Instant::now();
            if now >= deadline {
                return Err("HTTP request timed out waiting for the response".into());
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
        }
    }
}
