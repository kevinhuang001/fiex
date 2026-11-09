//! Bounded single-flight cache for preflight availability queries.
use crate::core::fs::{ResourceBroker, changes};
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
        if request.phase != PreflightPhase::Availability || request.validate().is_err() {
            return false;
        }
        let key = AvailabilityCacheKey {
            path: self.path.clone(),
            resource_revision: changes::revision(&self.path),
            request: request.clone(),
        };
        let (cell, owns_check, cache_full) = match availability_cache().lock() {
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
                                matches!(&*status, AvailabilityStatus::Ready { .. })
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
            let allowed = self.check_preflight(None, request).is_ok();
            record_availability(request, false, started.elapsed(), allowed, &self.path);
            return allowed;
        }
        let Some(cell) = cell else {
            return self.check_preflight(None, request).is_ok();
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
                record_availability(request, true, Duration::ZERO, allowed, &self.path);
                return allowed;
            }
        }

        let started = Instant::now();
        let allowed = self.check_preflight(None, request).is_ok();
        *cell
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = AvailabilityStatus::Ready {
            created: Instant::now(),
            allowed,
        };
        cell.ready.notify_all();
        record_availability(request, false, started.elapsed(), allowed, &self.path);
        allowed
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
