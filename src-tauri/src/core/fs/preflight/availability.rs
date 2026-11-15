//! Bounded single-flight cache for preflight availability queries.
use crate::core::fs::{FsError, FsErrorKind, ResourceBroker, changes};
use crate::infrastructure::cache::lru::LruMap;
use crate::plugin_sdk::filesystem::{
    CreateFilePreflight, CreateFolderPreflight, PreflightOperation, PreflightPhase,
    PreflightRequest, RemovePreflight, RenamePreflight, TrashPreflight,
};
use std::{
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

const AVAILABILITY_CACHE_TTL: Duration = Duration::from_millis(300);
const AVAILABILITY_CACHE_LIMIT: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct AvailabilityCacheKey {
    path: String,
    resource_revision: u64,
    request: PreflightRequest,
}

enum AvailabilityStatus {
    Pending,
    // The last provider call failed transiently; the next caller retries.
    Retry,
    Ready { created: Instant, allowed: bool },
}

struct AvailabilityCell {
    status: Mutex<AvailabilityStatus>,
    ready: Condvar,
}

impl AvailabilityCell {
    fn pending() -> Self {
        Self {
            status: Mutex::new(AvailabilityStatus::Pending),
            ready: Condvar::new(),
        }
    }
}

fn availability_cache() -> &'static Mutex<LruMap<AvailabilityCacheKey, Arc<AvailabilityCell>>> {
    static CACHE: OnceLock<Mutex<LruMap<AvailabilityCacheKey, Arc<AvailabilityCell>>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

pub(crate) fn clear_availability_cache() {
    if let Ok(mut cache) = availability_cache().lock() {
        cache.clear();
    }
}

impl ResourceBroker {
    pub(crate) fn review_availability_cached(&self, request: &PreflightRequest) -> bool {
        review_cached(&self.path, changes::revision(&self.path), request, || {
            self.check_preflight(None, request)
        })
    }

    pub(crate) fn review_create_available(&self, folder: bool) -> bool {
        let operation = if folder {
            PreflightOperation::CreateFolder(CreateFolderPreflight {
                parent_id: self.path.clone(),
                name: None,
            })
        } else {
            PreflightOperation::CreateFile(CreateFilePreflight {
                parent_id: self.path.clone(),
                name: None,
            })
        };
        self.review_availability_cached(&PreflightRequest::new(
            PreflightPhase::Availability,
            operation,
        ))
    }

    pub(crate) fn review_rename_available(&self) -> bool {
        self.review_availability_cached(&PreflightRequest::new(
            PreflightPhase::Availability,
            PreflightOperation::Rename(RenamePreflight {
                node_id: self.path.clone(),
                new_name: None,
            }),
        ))
    }

    pub(crate) fn review_trash_available(&self) -> bool {
        self.review_availability_cached(&PreflightRequest::new(
            PreflightPhase::Availability,
            PreflightOperation::Trash(TrashPreflight {
                node_ids: vec![self.path.clone()],
            }),
        ))
    }

    pub(crate) fn review_remove_available(&self) -> bool {
        self.review_availability_cached(&PreflightRequest::new(
            PreflightPhase::Availability,
            PreflightOperation::Remove(RemovePreflight {
                node_ids: vec![self.path.clone()],
            }),
        ))
    }
}

fn review_cached(
    path: &str,
    resource_revision: u64,
    request: &PreflightRequest,
    check: impl Fn() -> Result<(), FsError>,
) -> bool {
    review_cached_with_cache(
        availability_cache(),
        path,
        resource_revision,
        request,
        check,
    )
}

fn review_cached_with_cache(
    cache: &Mutex<LruMap<AvailabilityCacheKey, Arc<AvailabilityCell>>>,
    path: &str,
    resource_revision: u64,
    request: &PreflightRequest,
    check: impl Fn() -> Result<(), FsError>,
) -> bool {
    if request.phase != PreflightPhase::Availability || request.validate().is_err() {
        return false;
    }
    let key = AvailabilityCacheKey {
        path: path.to_owned(),
        resource_revision,
        request: request.clone(),
    };
    let (cell, owns_check, cache_full) = match cache.lock() {
        Ok(mut cache) => {
            if let Some(cell) = cache.get_mut(&key) {
                (Some(Arc::clone(cell)), false, false)
            } else {
                while cache.len() >= AVAILABILITY_CACHE_LIMIT {
                    let evicted = cache
                        .pop_lru_where(|_, cell| {
                            let status = cell
                                .status
                                .lock()
                                .unwrap_or_else(|error| error.into_inner());
                            matches!(
                                &*status,
                                AvailabilityStatus::Retry | AvailabilityStatus::Ready { .. }
                            )
                        })
                        .is_some();
                    if !evicted {
                        break;
                    }
                }
                if cache.len() >= AVAILABILITY_CACHE_LIMIT {
                    (None, false, true)
                } else {
                    let cell = Arc::new(AvailabilityCell::pending());
                    let _ = cache.insert(key, Arc::clone(&cell));
                    (Some(cell), true, false)
                }
            }
        }
        Err(_) => (None, false, true),
    };

    if cache_full {
        let started = Instant::now();
        let allowed = check().is_ok();
        record_availability(request, false, started.elapsed(), allowed, path);
        return allowed;
    }
    let Some(cell) = cell else {
        return check().is_ok();
    };

    if !owns_check {
        let mut status = cell
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let cached = loop {
            match &*status {
                AvailabilityStatus::Pending => {
                    status = cell
                        .ready
                        .wait(status)
                        .unwrap_or_else(|error| error.into_inner());
                }
                AvailabilityStatus::Retry => {
                    *status = AvailabilityStatus::Pending;
                    break None;
                }
                AvailabilityStatus::Ready { created, allowed }
                    if created.elapsed() < AVAILABILITY_CACHE_TTL =>
                {
                    break Some(*allowed);
                }
                AvailabilityStatus::Ready { .. } => {
                    *status = AvailabilityStatus::Pending;
                    break None;
                }
            }
        };
        if let Some(allowed) = cached {
            drop(status);
            record_availability(request, true, Duration::ZERO, allowed, path);
            return allowed;
        }
    }

    let started = Instant::now();
    let result = check();
    let allowed = result.is_ok();
    let next_status = match result {
        Ok(()) => AvailabilityStatus::Ready {
            created: Instant::now(),
            allowed: true,
        },
        Err(error) if is_retryable(&error) => AvailabilityStatus::Retry,
        Err(_) => AvailabilityStatus::Ready {
            created: Instant::now(),
            allowed: false,
        },
    };
    *cell
        .status
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = next_status;
    cell.ready.notify_all();
    record_availability(request, false, started.elapsed(), allowed, path);
    allowed
}

fn is_retryable(error: &FsError) -> bool {
    // Cache policy denials, but never let a disconnected or unhealthy provider
    // leave the UI disabled for the full availability TTL after it recovers.
    matches!(
        error.kind,
        FsErrorKind::Offline | FsErrorKind::MountNotFound | FsErrorKind::Internal
    )
}

fn record_availability(
    request: &PreflightRequest,
    cache_hit: bool,
    elapsed: Duration,
    allowed: bool,
    path: &str,
) {
    static TRACE: OnceLock<bool> = OnceLock::new();
    if !*TRACE.get_or_init(|| std::env::var_os("FIEX_TRACE_PREFLIGHT").is_some()) {
        return;
    }
    let transport = if crate::core::fs::provider_registry::resource_access(path)
        .is_ok_and(|access| access == crate::core::fs::provider_registry::ResourceAccess::Provider)
    {
        "ipc"
    } else {
        "native"
    };
    let event = serde_json::json!({
        "operation": request.op().as_str(),
        "phase": "availability",
        "transport": transport,
        "elapsedMs": elapsed.as_millis(),
        "cacheHit": cache_hit,
        "decision": if allowed { "allow" } else { "deny" },
    });
    crate::infrastructure::logging::record_core(
        "debug",
        "host.fs.preflight.availability",
        &event.to_string(),
    );
}

#[cfg(test)]
mod tests;
