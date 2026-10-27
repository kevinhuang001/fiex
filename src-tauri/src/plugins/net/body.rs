use super::*;

pub(super) struct PipeState {
    pub(super) chunks: VecDeque<Vec<u8>>,
    pub(super) offset: usize,
    pub(super) eof: bool,
    /// The worker no longer consumes bytes (request finished or failed).
    pub(super) closed: bool,
    /// Cancelled by close/process teardown; carries the reason.
    pub(super) abort: Option<String>,
    pub(super) read_error: Option<String>,
    pub(super) accepted: u64,
    pub(super) delivered: u64,
}

/// Bounded producer/consumer pipe feeding `reqwest::blocking::Body::sized`.
/// The reader blocks (with a deadline) instead of buffering the body.
pub(super) struct BodyPipe {
    pub(super) state: Mutex<PipeState>,
    pub(super) changed: Condvar,
    pub(super) content_length: u64,
    pub(super) deadline: Instant,
}

impl BodyPipe {
    pub(super) fn new(content_length: u64, deadline: Instant) -> Self {
        Self {
            state: Mutex::new(PipeState {
                chunks: VecDeque::new(),
                offset: 0,
                eof: false,
                closed: false,
                abort: None,
                read_error: None,
                accepted: 0,
                delivered: 0,
            }),
            changed: Condvar::new(),
            content_length,
            deadline,
        }
    }

    pub(super) fn state(&self) -> std::sync::MutexGuard<'_, PipeState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Accept one chunk. `eof` closes the body after the chunk. The call waits
    /// only while the bounded pipe is full; a finished or failed request makes
    /// it fail instead of blocking forever.
    pub(super) fn push(&self, bytes: Vec<u8>, eof: bool) -> Result<u64, String> {
        if bytes.len() > MAX_BODY_CHUNK {
            return Err("HTTP body chunk exceeds 4 MiB".into());
        }
        let mut state = self.state();
        loop {
            if let Some(reason) = &state.abort {
                return Err(reason.clone());
            }
            if state.eof {
                return Err("HTTP request body already ended".into());
            }
            let next_total = state
                .accepted
                .checked_add(bytes.len() as u64)
                .ok_or("HTTP request body size overflow")?;
            if next_total > self.content_length {
                return Err(format!(
                    "HTTP request body exceeds contentLength ({} > {})",
                    next_total, self.content_length
                ));
            }
            // The request may complete (and close the pipe) after exactly
            // contentLength bytes; a late final `eof` is still meaningful.
            if bytes.is_empty() && eof && next_total == self.content_length {
                state.eof = true;
                drop(state);
                self.changed.notify_all();
                return Ok(next_total);
            }
            if state.closed {
                return Err("HTTP request body is closed".into());
            }
            if bytes.is_empty() {
                if eof {
                    state.eof = true;
                }
                drop(state);
                self.changed.notify_all();
                return Ok(next_total);
            }
            if state.chunks.len() < BODY_PIPE_CHUNKS {
                state.accepted = next_total;
                state.chunks.push_back(bytes);
                if eof {
                    state.eof = true;
                }
                drop(state);
                self.changed.notify_all();
                return Ok(next_total);
            }
            let now = Instant::now();
            if now >= self.deadline {
                return Err("HTTP request body timed out before contentLength was reached".into());
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, self.deadline - now)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
        }
    }

    pub(super) fn close(&self) {
        let mut state = self.state();
        state.closed = true;
        drop(state);
        self.changed.notify_all();
    }

    pub(super) fn abort(&self, reason: &str) {
        let mut state = self.state();
        if state.abort.is_none() {
            state.abort = Some(reason.to_owned());
        }
        state.closed = true;
        drop(state);
        self.changed.notify_all();
    }

    pub(super) fn read(&self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let mut state = self.state();
        loop {
            if let Some(reason) = &state.abort {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, reason.clone()));
            }
            if state.delivered >= self.content_length {
                return Ok(0);
            }
            if !state.chunks.is_empty() {
                let count = (state.chunks[0].len() - state.offset)
                    .min(output.len())
                    .min((self.content_length - state.delivered) as usize);
                {
                    let chunk = &state.chunks[0];
                    output[..count].copy_from_slice(&chunk[state.offset..state.offset + count]);
                }
                state.offset += count;
                state.delivered += count as u64;
                if state.offset == state.chunks[0].len() {
                    state.chunks.pop_front();
                    state.offset = 0;
                }
                drop(state);
                self.changed.notify_all();
                return Ok(count);
            }
            if state.eof {
                let message = format!(
                    "HTTP request body ended after {} of {} bytes",
                    state.delivered, self.content_length
                );
                state.read_error = Some(message.clone());
                return Err(io::Error::new(io::ErrorKind::InvalidData, message));
            }
            if state.closed {
                let message = "HTTP request body is closed".to_string();
                state.read_error = Some(message.clone());
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, message));
            }
            let now = Instant::now();
            if now >= self.deadline {
                let message =
                    "HTTP request body timed out before contentLength was reached".to_string();
                state.read_error = Some(message.clone());
                return Err(io::Error::new(io::ErrorKind::TimedOut, message));
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, self.deadline - now)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
        }
    }

    pub(super) fn read_error(&self) -> Option<String> {
        self.state().read_error.clone()
    }
}

pub(super) struct BodyReader {
    pub(super) pipe: Arc<BodyPipe>,
}

impl Read for BodyReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.pipe.read(output)
    }
}
