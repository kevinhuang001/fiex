//! Application adapters that resolve trusted resources through the live FS
//! and VFS registries. Resource data and projections live in `fiex-resource`.
use crate::{core::files::Entry, plugins::contracts::ResourceMetadata};
use fiex_provider_contracts::{EventContext, FsOperation, ResourceContext};
use std::path::{Path, PathBuf};

pub use fiex_resource::ResourceSnapshot;

pub fn new_resource_snapshot(display: &str, local: &Path) -> ResourceSnapshot {
    let mount_id = crate::core::fs::provider_registry::reference_mount_id(display)
        .unwrap_or_else(|| "local".into());
    let name = local
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| local.to_string_lossy().into_owned());
    let extension = Path::new(&name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    ResourceSnapshot::new(
        display,
        local,
        &mount_id,
        &fiex_resource::mime_for(&extension),
    )
}

pub fn from_metadata(display: &str, metadata: ResourceMetadata) -> ResourceSnapshot {
    let mount_id = crate::core::fs::provider_registry::reference_mount_id(display)
        .unwrap_or_else(|| "local".into());
    let local = crate::core::volumes::native_vfs::backing_path(display)
        .unwrap_or_else(|| PathBuf::from(display));
    ResourceSnapshot::from_metadata(display, local, &mount_id, metadata)
}

pub fn from_entry(display: &str, entry: &Entry) -> ResourceSnapshot {
    let mut resource = from_metadata(display, entry_metadata(entry));
    resource.storage_projection = entry.storage_projection.clone();
    resource.operations = Some(crate::plugins::resource::file_capabilities::for_entry(
        display, entry,
    ));
    resource
}

pub fn entry_metadata(entry: &Entry) -> ResourceMetadata {
    ResourceMetadata {
        name: entry.name.clone(),
        kind: if entry.is_symlink {
            "symlink"
        } else if entry.is_dir {
            "directory"
        } else {
            "file"
        }
        .into(),
        extension: Some(entry.extension.clone()),
        mime: Some(entry.mime.clone()),
        size: (!entry.is_dir).then_some(entry.size),
        modified: Some(entry.modified as i64),
    }
}

pub fn resolve(display: &str) -> Result<ResourceSnapshot, String> {
    match crate::core::fs::provider_registry::resource_access(display).map_err(String::from)? {
        crate::core::fs::provider_registry::ResourceAccess::HostPath => {
            Ok(new_resource_snapshot(display, Path::new(display)))
        }
        crate::core::fs::provider_registry::ResourceAccess::Provider => {
            let entry = crate::core::fs::provider_registry::resource_entry(display)?;
            Ok(from_entry(display, &entry))
        }
    }
}

/// Reuse a trusted directory snapshot for metadata, then fall back to the live
/// resource route. Content operations still reauthorize through the broker.
pub fn resolve_from_directory_snapshot(display: &str) -> Result<ResourceSnapshot, String> {
    if crate::core::fs::provider_registry::resource_access(display).map_err(String::from)?
        == crate::core::fs::provider_registry::ResourceAccess::Provider
        && let Ok(parent) = crate::core::fs::provider_registry::parent_path(display)
        && let Ok(Some(snapshot)) =
            crate::core::fs::directory_cache::read_snapshot(&parent, true, &[])
        && let Some(entry) = snapshot
            .listing
            .entries
            .into_iter()
            .find(|entry| entry.path == display)
    {
        return Ok(from_entry(display, &entry));
    }
    resolve(display)
}

/// Keep the app-specific authorization/presentation projection outside the
/// portable resource model.
pub trait ResourceSnapshotProjection {
    fn project(
        &self,
        owner: &str,
        session: &str,
        metadata_allowed: bool,
        read_allowed: bool,
    ) -> ResourceContext;

    fn context(&self, owner: &str, metadata_allowed: bool, read_allowed: bool) -> EventContext;

    fn export_path(
        &self,
        user_action: crate::core::fs::mapped_access::UserActionContext,
    ) -> Result<PathBuf, String>;
}

impl ResourceSnapshotProjection for ResourceSnapshot {
    fn project(
        &self,
        _owner: &str,
        _session: &str,
        metadata_allowed: bool,
        _read_allowed: bool,
    ) -> ResourceContext {
        let (view, own_storage) = crate::plugins::match_rules::typed_locations_for_snapshot(self);
        let storage = match &self.storage_projection {
            fiex_resource::StorageProjection::Owned => own_storage,
            fiex_resource::StorageProjection::Projected(source) => source.clone(),
        };
        let capabilities = if metadata_allowed {
            let effective = self.operations.unwrap_or_else(|| {
                crate::plugins::resource::file_capabilities::for_path(&self.display)
            });
            fiex_provider_contracts::ResourceCapabilities {
                operations: FsOperation::ALL
                    .into_iter()
                    .filter(|operation| effective.allows(*operation))
                    .collect(),
            }
        } else {
            fiex_provider_contracts::ResourceCapabilities::default()
        };
        self.project_data(metadata_allowed, view, storage, capabilities)
    }

    fn context(&self, owner: &str, metadata_allowed: bool, read_allowed: bool) -> EventContext {
        let session_id = crate::plugins::mounts::new_id().unwrap_or_default();
        self.event_context(
            session_id.clone(),
            self.project(owner, &session_id, metadata_allowed, read_allowed),
        )
    }

    fn export_path(
        &self,
        user_action: crate::core::fs::mapped_access::UserActionContext,
    ) -> Result<PathBuf, String> {
        let paths = crate::core::fs::export_for_external_drag(
            std::slice::from_ref(&self.display),
            user_action,
        )?;
        paths
            .into_iter()
            .next()
            .map(PathBuf::from)
            .ok_or("missing export".into())
    }
}

mod binding;
pub use binding::{ResourceBinding, bind_selected};

#[cfg(test)]
mod tests;
