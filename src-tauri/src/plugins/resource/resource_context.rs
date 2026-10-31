//! Core-owned resource snapshot shared by Info and Preview providers.
//! Metadata is retrieved once, then projected separately for each provider.
//! The snapshot never crosses the plugin process boundary.
use crate::plugins::contracts::{EventContext, ResourceContext, ResourceMetadata, ResourceRef};
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
    pub read_only: bool,
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
        let read_only = stat
            .as_ref()
            .is_some_and(crate::core::files::metadata_readonly);
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
            read_only,
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
        let mount_id = crate::plugins::mounts::parse(display)
            .map(|uri| uri.mount_id)
            .unwrap_or_else(|_| {
                if let Some(route) = crate::usbfs::route(display) {
                    return format!(
                        "core:usb:{}",
                        crate::usbfs::generation(&route).unwrap_or_else(|_| "offline".into())
                    );
                }
                crate::core::volumes::virtual_resolvers::view_for(display)
                    .map(|provider| provider.id().to_owned())
                    .unwrap_or_else(|| "local".into())
            });
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
        if crate::core::volumes::virtual_resolvers::opaque(display) {
            let entry = crate::core::volumes::virtual_resolvers::resource_entry(display)?;
            return Ok(Self::from_entry(display, &entry));
        }
        if let Some(route) = crate::usbfs::route(display) {
            let entry = crate::usbfs::entry(&route)?;
            return Ok(Self::from_entry(display, &entry));
        }
        if !crate::plugins::mounts::handles(display) {
            // Share ResourceBroker's trusted owner resolution. Unsupported
            // schemes may not masquerade as OS-local files during metadata
            // discovery or provider candidate matching.
            if !matches!(
                crate::core::fs::provider_registry::identity(display),
                Some(("fs", provider, _)) if provider == "core:local-fs"
            ) {
                return Err("Unsupported: unregistered filesystem URI".into());
            }
            return Ok(Self::new(display, Path::new(display)));
        }
        let entry = if crate::remote::is_remote(display) {
            crate::remote::stat(display)?
        } else {
            crate::plugins::vfs::stat(display)?
        };
        Ok(Self::from_entry(display, &entry))
    }
    pub fn from_entry(display: &str, entry: &crate::core::files::Entry) -> Self {
        let mut resource = Self::from_metadata(display, Self::entry_metadata(entry));
        if crate::core::volumes::applications::is_user(display) {
            if let Ok(path) = crate::core::volumes::applications::stored_path(display) {
                resource.local = path;
            }
        }
        resource.read_only = entry.readonly;
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
        Self {
            display: display.into(),
            local: PathBuf::from(display),
            reference,
            stat: None,
            is_dir: metadata.kind == "directory",
            read_only: false,
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
        ResourceContext {
            reference: self.reference.clone(),
            metadata: metadata_allowed.then(|| self.metadata.clone()).flatten(),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_schemes_fail_before_local_metadata_discovery() {
        for path in ["unknown://not-registered", "unknown:opaque-reference"] {
            assert!(ResourceSnapshot::resolve(path).is_err(), "{path}");
        }
    }

    #[test]
    fn metadata_projection_does_not_grant_read_and_reuses_identity() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("example.txt");
        fs::write(&file, b"abc").unwrap();
        let snapshot = ResourceSnapshot::new(file.to_str().unwrap(), &file);
        let without_grant = snapshot.context("org.example", false, false);
        let with_meta = snapshot.context("org.example", true, false);
        assert_eq!(
            without_grant.targets[0].reference,
            with_meta.targets[0].reference
        );
        assert!(without_grant.targets[0].metadata.is_none());
        assert_eq!(
            with_meta.targets[0].metadata.as_ref().unwrap().size,
            Some(3)
        );
    }
}

/// Private Core-UI binding. Plugin iframe SDKs receive only `reference`; the
/// path travels out of band and must be inside this invocation's selection.
#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceBinding {
    #[serde(rename = "ref")]
    pub reference: ResourceRef,
    pub path: String,
}
pub fn bind_selected(
    scope: &mut crate::plugins::resource::fs_access::Scope,
    bindings: &[ResourceBinding],
) -> Result<(), String> {
    for binding in bindings {
        if crate::plugins::mounts::handles(&binding.path)
            || crate::usbfs::route(&binding.path).is_some()
            || crate::core::volumes::virtual_resolvers::opaque(&binding.path)
        {
            let snapshot = ResourceSnapshot::resolve(&binding.path)?;
            if snapshot.reference != binding.reference {
                return Err("ScopeDenied: resource reference does not match selected node".into());
            }
            scope.bind_remote(
                &serde_json::json!({"ref":binding.reference,"metadata":snapshot.metadata()}),
                &snapshot,
            )?;
            continue;
        }
        // An attacker-controlled resource binding is never promoted to a
        // local path merely because its scheme has no registered mount.
        if !matches!(
            crate::core::fs::provider_registry::identity(&binding.path),
            Some(("fs", provider, _)) if provider == "core:local-fs"
        ) {
            return Err("ScopeDenied: unsupported resource owner".into());
        }
        let path = std::path::Path::new(&binding.path);
        scope.authorize(&binding.path)?;
        let snapshot = ResourceSnapshot::new(&binding.path, path);
        if snapshot.reference != binding.reference {
            return Err("ScopeDenied: resource reference does not match selected node".into());
        }
        scope.bind_resource(
            &serde_json::json!({"ref":binding.reference,"metadata":snapshot.metadata()}),
            Some(path),
        )?;
    }
    Ok(())
}
