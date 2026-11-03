//! One operation-specific policy gate shared by built-in actions and transfer
//! flows. Capabilities remain the authorization boundary; Preflight can only
//! add a dynamic restriction.
use super::*;
use crate::infrastructure::cache::lru::LruMap;
use crate::plugin_sdk::filesystem::{
    CreateFilePreflight, CreateFolderPreflight, ExportPreflight, ImportPreflight, MovePreflight,
    PreflightOperation, PreflightPhase, PreflightRequest, RemovePreflight, RenamePreflight,
    TransferKind, TrashPreflight,
};
use fiex_provider_contracts::TransferMetadata;
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
    pub(crate) fn transfer_metadata(&self) -> Result<TransferMetadata, FsError> {
        let stat = self.stat()?;
        let entry = self.desktop_entry()?;
        let system = match crate::core::fs::provider_registry::resource_access(&self.path) {
            Ok(crate::core::fs::provider_registry::ResourceAccess::HostPath) => Some(
                crate::core::volumes::applications::is_system_app(&self.path)
                    || crate::core::volumes::application_policy::protected(&self.path),
            ),
            Ok(crate::core::fs::provider_registry::ResourceAccess::Provider)
                if crate::core::volumes::applications::is_user(&self.path)
                    || crate::core::volumes::applications::is_system_app(&self.path) =>
            {
                Some(crate::core::volumes::applications::is_system_app(
                    &self.path,
                ))
            }
            _ => None,
        };
        Ok(TransferMetadata {
            name: entry.name,
            kind: if entry.is_dir {
                "directory"
            } else if entry.is_symlink {
                "symlink"
            } else {
                "file"
            }
            .into(),
            size: stat["size"].as_u64(),
            modified: stat["modified"].as_u64(),
            extension: Some(entry.extension),
            mime: Some(entry.mime),
            executable_shortcut: executable_shortcut(&self.path, entry.link_target.as_deref()),
            system,
        })
    }

    pub(crate) fn check_preflight(
        &self,
        destination: Option<&Self>,
        request: &PreflightRequest,
    ) -> Result<(), FsError> {
        request.validate().map_err(FsError::internal)?;
        provider_registry::preflight(self, destination, request)
    }

    pub(crate) fn review_availability_cached(&self, request: &PreflightRequest) -> bool {
        if request.phase != PreflightPhase::Availability || request.validate().is_err() {
            return false;
        }
        let key = AvailabilityCacheKey {
            path: self.path.clone(),
            resource_revision: super::changes::revision(&self.path),
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

    pub(crate) fn review_create(
        &self,
        folder: bool,
        name: Option<&str>,
        phase: PreflightPhase,
    ) -> Result<(), FsError> {
        let operation = if folder {
            PreflightOperation::CreateFolder(CreateFolderPreflight {
                parent_id: self.path.clone(),
                name: name.map(str::to_owned),
            })
        } else {
            PreflightOperation::CreateFile(CreateFilePreflight {
                parent_id: self.path.clone(),
                name: name.map(str::to_owned),
            })
        };
        self.check_preflight(None, &PreflightRequest::new(phase, operation))
    }

    pub(crate) fn review_create_child(
        &self,
        folder: bool,
        phase: PreflightPhase,
    ) -> Result<(), FsError> {
        let parent = self.parent_broker()?;
        let name = self
            .path
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| FsError::new(FsErrorKind::InvalidVfsUri, "child name is missing"))?;
        parent.review_create(folder, Some(name), phase)
    }

    pub(crate) fn parent_broker(&self) -> Result<Self, FsError> {
        let parent_path = binding::parent_path(&self.path)?;
        let parent_ref = self.scope.reference_for_path(&parent_path)?;
        ResourceBroker::resolve(&self.scope, &self.owner, &parent_ref)
    }

    pub(crate) fn review_rename(
        &self,
        new_name: Option<&str>,
        phase: PreflightPhase,
    ) -> Result<(), FsError> {
        self.check_preflight(
            None,
            &PreflightRequest::new(
                phase,
                PreflightOperation::Rename(RenamePreflight {
                    node_id: self.path.clone(),
                    new_name: new_name.map(str::to_owned),
                }),
            ),
        )
    }

    pub(crate) fn review_remove(&self, phase: PreflightPhase) -> Result<(), FsError> {
        self.check_preflight(
            None,
            &PreflightRequest::new(
                phase,
                PreflightOperation::Remove(RemovePreflight {
                    node_ids: vec![self.path.clone()],
                }),
            ),
        )
    }

    pub(crate) fn review_trash(&self, phase: PreflightPhase) -> Result<(), FsError> {
        self.check_preflight(
            None,
            &PreflightRequest::new(
                phase,
                PreflightOperation::Trash(TrashPreflight {
                    node_ids: vec![self.path.clone()],
                }),
            ),
        )
    }

    pub(crate) fn review_move(
        &self,
        target_parent: &Self,
        target_name: Option<&str>,
        phase: PreflightPhase,
    ) -> Result<(), FsError> {
        self.check_preflight(
            Some(target_parent),
            &PreflightRequest::new(
                phase,
                PreflightOperation::Move(MovePreflight {
                    node_id: self.path.clone(),
                    target_parent_id: target_parent.path.clone(),
                    target_name: target_name.map(str::to_owned),
                }),
            ),
        )
    }

    pub(crate) fn review_import(
        &self,
        transfer_kind: TransferKind,
        name: &str,
        metadata: &TransferMetadata,
        phase: PreflightPhase,
    ) -> Result<(), FsError> {
        self.check_preflight(
            None,
            &PreflightRequest::new(
                phase,
                PreflightOperation::Import(ImportPreflight {
                    parent_id: self.path.clone(),
                    name: Some(name.into()),
                    source: metadata.clone(),
                    transfer_kind,
                }),
            ),
        )
    }

    pub(crate) fn review_export(
        &self,
        transfer_kind: TransferKind,
        phase: PreflightPhase,
    ) -> Result<(), FsError> {
        self.check_preflight(
            None,
            &PreflightRequest::new(
                phase,
                PreflightOperation::Export(ExportPreflight {
                    node_id: self.path.clone(),
                    transfer_kind,
                }),
            ),
        )
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

fn executable_shortcut(path: &str, target: Option<&str>) -> Option<bool> {
    if crate::core::volumes::applications::is_user(path) {
        return Some(crate::core::volumes::applications::is_user_application(
            path,
        ));
    }
    // Provider paths carry no trusted host-path or executable-target authority.
    if !crate::core::fs::provider_registry::resource_access(path)
        .is_ok_and(|access| access == crate::core::fs::provider_registry::ResourceAccess::HostPath)
    {
        return None;
    }
    Some(crate::core::volumes::applications::classify_host_application(path, target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applications_rejects_document_import_before_any_write_or_source_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("preflight-document.txt");
        std::fs::write(&file, b"keep source").unwrap();
        let result = crate::core::fs::transfer(
            &[file.to_string_lossy().into_owned()],
            crate::core::volumes::applications::URI,
            true,
            false,
            &[],
            &[],
            &std::sync::atomic::AtomicBool::new(false),
            &|_, _, _| {},
        );
        assert!(result.is_err(), "document import must be rejected");
        assert_eq!(std::fs::read(&file).unwrap(), b"keep source");
    }

    #[test]
    fn same_mount_transfer_checks_are_not_skipped() {
        let view = crate::core::fs::binding::trusted(
            crate::core::volumes::applications::URI,
            "core:test-preflight",
        )
        .unwrap();
        let metadata = TransferMetadata {
            name: "document.txt".into(),
            kind: "file".into(),
            size: Some(12),
            modified: None,
            extension: Some("txt".into()),
            mime: None,
            executable_shortcut: Some(false),
            system: Some(false),
        };
        assert!(view
            .review_import(
                TransferKind::Copy,
                "document.txt",
                &metadata,
                PreflightPhase::Execute,
            )
            .is_err());
    }

    #[test]
    fn local_metadata_preserves_known_negative_classifications() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-an-app.txt");
        std::fs::write(&file, b"data").unwrap();
        let resource =
            crate::core::fs::binding::trusted(&file.to_string_lossy(), "core:test-preflight")
                .unwrap();
        let metadata = resource.transfer_metadata().unwrap();
        assert_eq!(metadata.executable_shortcut, Some(false));
        assert_eq!(metadata.system, Some(false));
    }
}
