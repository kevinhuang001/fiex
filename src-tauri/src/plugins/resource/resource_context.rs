//! Core-owned resource snapshot shared by Info and Preview providers.
//! Metadata is retrieved once, then projected separately for each provider.
//! The snapshot never crosses the plugin process boundary.
use crate::plugins::contracts::{EventContext, ResourceContext, ResourceMetadata, ResourceRef};
use fiex_provider_contracts::ResourceCapabilities;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct ResourceSnapshot {
    pub display: String,
    pub local: PathBuf,
    pub reference: ResourceRef,
    pub stat: Option<fs::Metadata>,
    pub is_dir: bool,
    pub ext: String,
    pub mime: String,
    pub name: String,
    metadata: Option<ResourceMetadata>,
    pub operations: Option<crate::plugins::resource::file_capabilities::FileCapabilities>,
}

impl ResourceSnapshot {
    pub fn new(display: &str, local: &Path) -> Self {
        let stat = fs::symlink_metadata(local).ok();
        let is_dir = stat.as_ref().is_some_and(fs::Metadata::is_dir);
        let name = local
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| local.to_string_lossy().into_owned());
        let ext = Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mime = crate::plugins::match_rules::mime_for(&ext);
        let reference = Self::reference_for(display);
        let metadata = stat.as_ref().map(|stat| ResourceMetadata {
            name: name.clone(),
            kind: if stat.file_type().is_symlink() {
                "symlink"
            } else if is_dir {
                "directory"
            } else {
                "file"
            }
            .into(),
            extension: (!ext.is_empty()).then(|| ext.clone()),
            mime: (!ext.is_empty()).then(|| mime.clone()),
            size: (!is_dir).then_some(stat.len()),
            modified: stat
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_secs() as i64),
        });
        Self {
            display: display.into(),
            local: local.to_path_buf(),
            reference,
            stat,
            is_dir,
            ext,
            mime,
            name,
            metadata,
            operations: None,
        }
    }

    /// Stable for this Core process; the keyed digest cannot reveal a host path.
    /// Identity is never an authorization token: each request still needs Scope.
    fn reference_for(display: &str) -> ResourceRef {
        use sha2::{Digest, Sha256};
        use std::sync::OnceLock;
        static KEY: OnceLock<[u8; 32]> = OnceLock::new();
        let key = KEY.get_or_init(|| {
            let mut key = [0; 32];
            getrandom::getrandom(&mut key).expect("Core resource identity entropy unavailable");
            key
        });
        let mount_id = crate::core::fs::provider_registry::reference_mount_id(display)
            .unwrap_or_else(|| "local".into());
        let mut digest = Sha256::new();
        digest.update(key);
        digest.update(mount_id.as_bytes());
        digest.update([0]);
        digest.update(display.as_bytes());
        ResourceRef {
            mount_id,
            node_id: format!("node:{}", hex::encode(digest.finalize())),
        }
    }

    pub fn metadata(&self) -> Option<&ResourceMetadata> {
        self.metadata.as_ref()
    }

    pub fn resolve(display: &str) -> Result<Self, String> {
        match crate::core::fs::provider_registry::resource_access(display).map_err(String::from)? {
            crate::core::fs::provider_registry::ResourceAccess::HostPath => {
                Ok(Self::new(display, Path::new(display)))
            }
            crate::core::fs::provider_registry::ResourceAccess::Provider => {
                let entry = crate::core::fs::provider_registry::resource_entry(display)?;
                Ok(Self::from_entry(display, &entry))
            }
        }
    }

    /// Metadata reads may reuse the Core-authorized listing snapshot for VFS
    /// resources. Content reads still go through the broker and reauthorize.
    pub fn resolve_from_directory_snapshot(display: &str) -> Result<Self, String> {
        if crate::core::fs::provider_registry::resource_access(display).map_err(String::from)?
            == crate::core::fs::provider_registry::ResourceAccess::Provider
        {
            if let Ok(parent) = crate::core::fs::provider_registry::parent_path(display) {
                if let Ok(Some(snapshot)) =
                    crate::core::fs::directory_cache::read_snapshot(&parent, true, &[])
                {
                    if let Some(entry) = snapshot
                        .listing
                        .entries
                        .into_iter()
                        .find(|entry| entry.path == display)
                    {
                        return Ok(Self::from_entry(display, &entry));
                    }
                }
            }
        }
        Self::resolve(display)
    }
    pub fn from_entry(display: &str, entry: &crate::core::files::Entry) -> Self {
        let mut resource = Self::from_metadata(display, Self::entry_metadata(entry));
        resource.operations = Some(crate::plugins::resource::file_capabilities::for_entry(
            display, entry,
        ));
        resource
    }
    pub fn entry_metadata(entry: &crate::core::files::Entry) -> ResourceMetadata {
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
    pub fn from_metadata(display: &str, metadata: ResourceMetadata) -> Self {
        let reference = Self::reference_for(display);
        let local = crate::core::volumes::native_vfs::backing_path(display)
            .unwrap_or_else(|| PathBuf::from(display));
        Self {
            display: display.into(),
            local,
            reference,
            stat: None,
            is_dir: metadata.kind == "directory",
            ext: metadata.extension.clone().unwrap_or_default(),
            mime: metadata.mime.clone().unwrap_or_default(),
            name: metadata.name.clone(),
            metadata: Some(metadata),
            operations: None,
        }
    }
    /// A native renderer needs a local path. Export through the same bounded,
    /// leased broker as external open/drag; never expose provider caches.
    pub fn export_path(&self) -> Result<PathBuf, String> {
        let paths = crate::core::fs::export_for_external_drag(std::slice::from_ref(&self.display))?;
        paths
            .into_iter()
            .next()
            .map(PathBuf::from)
            .ok_or("missing export".into())
    }
    pub fn project(
        &self,
        _owner: &str,
        _session: &str,
        metadata_allowed: bool,
        _read_allowed: bool,
    ) -> ResourceContext {
        let (view, storage) = crate::plugins::match_rules::typed_locations_for(&self.display);
        let capabilities = if metadata_allowed {
            let effective = self.operations.unwrap_or_else(|| {
                crate::plugins::resource::file_capabilities::for_path(&self.display)
            });
            ResourceCapabilities {
                operations: crate::plugins::provider_registry::FsOperation::ALL
                    .into_iter()
                    .filter(|operation| effective.allows(*operation))
                    .collect(),
            }
        } else {
            ResourceCapabilities::default()
        };
        ResourceContext {
            reference: self.reference.clone(),
            metadata: metadata_allowed.then(|| self.metadata.clone()).flatten(),
            view,
            storage,
            capabilities,
        }
    }
    pub fn context(&self, owner: &str, metadata_allowed: bool, read_allowed: bool) -> EventContext {
        let session_id = crate::plugins::mounts::new_id().unwrap_or_default();
        EventContext {
            targets: vec![self.project(owner, &session_id, metadata_allowed, read_allowed)],
            session_id,
        }
    }
}

mod binding;
pub use binding::{bind_selected, ResourceBinding};

#[cfg(test)]
mod tests;
