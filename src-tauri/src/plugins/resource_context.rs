//! Core-owned resource snapshot shared by Info and Preview providers.
//! Metadata is retrieved once, then projected separately for each provider.
//! The snapshot never crosses the plugin process boundary.
use crate::plugin_contracts::{EventContext, ResourceContext, ResourceMetadata, ResourceRef};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct ResourceSnapshot {
    content: std::sync::Arc<std::sync::OnceLock<Result<PathBuf, String>>>,
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
}

impl ResourceSnapshot {
    pub fn new(display: &str, local: &Path) -> Self {
        let stat = fs::symlink_metadata(local).ok();
        let is_dir = stat.as_ref().is_some_and(fs::Metadata::is_dir);
        let read_only = stat.as_ref().is_some_and(|s| s.permissions().readonly());
        let name = local
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| local.to_string_lossy().into_owned());
        let ext = Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mime = crate::plugin_match::mime_for(&ext);
        let reference = if crate::mounts::handles(display) {
            crate::mounts::parse(display)
                .map(|uri| ResourceRef {
                    mount_id: uri.mount_id,
                    node_id: format!("fiex-resource:{}", uri.relative),
                })
                .unwrap_or_else(|_| Self::local_ref())
        } else {
            Self::local_ref()
        };
        let metadata = stat.as_ref().map(|stat| ResourceMetadata {
            name: name.clone(),
            kind: if is_dir { "directory" } else { "file" }.into(),
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
            content: Default::default(),
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
        }
    }

    fn local_ref() -> ResourceRef {
        ResourceRef {
            mount_id: "local".into(),
            node_id: format!(
                "fiex-resource:{}",
                crate::mounts::new_id().unwrap_or_default()
            ),
        }
    }

    pub fn metadata(&self) -> Option<&ResourceMetadata> {
        self.metadata.as_ref()
    }

    pub fn resolve(display: &str) -> Result<Self, String> {
        if !crate::mounts::handles(display) {
            return Ok(Self::new(display, Path::new(display)));
        }
        let entry = if crate::remote::is_remote(display) {
            crate::remote::stat(display)?
        } else {
            crate::plugin_vfs::stat(display)?
        };
        let mut resource = Self::from_metadata(display, Self::entry_metadata(&entry));
        resource.read_only = entry.readonly;
        Ok(resource)
    }
    pub fn entry_metadata(entry: &crate::files::Entry) -> ResourceMetadata {
        ResourceMetadata {
            name: entry.name.clone(),
            kind: if entry.is_dir { "directory" } else { "file" }.into(),
            extension: Some(entry.extension.clone()),
            mime: Some(entry.mime.clone()),
            size: (!entry.is_dir).then_some(entry.size),
            modified: Some(entry.modified as i64),
        }
    }
    pub fn from_metadata(display: &str, metadata: ResourceMetadata) -> Self {
        let reference = ResourceRef {
            mount_id: crate::mounts::parse(display)
                .map(|uri| uri.mount_id)
                .unwrap_or_else(|_| "local".into()),
            node_id: Self::local_ref().node_id,
        };
        Self {
            content: Default::default(),
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
        }
    }
    pub fn materialize(&self) -> Result<PathBuf, String> {
        if crate::mounts::handles(&self.display) {
            crate::mounts::resolve(&self.display)?;
        }
        self.content
            .get_or_init(|| self.materialize_uncached())
            .clone()
    }
    fn materialize_uncached(&self) -> Result<PathBuf, String> {
        if !crate::mounts::handles(&self.display) {
            return Ok(self.local.clone());
        }
        if crate::remote::is_remote(&self.display) {
            if self.is_dir {
                futures::executor::block_on(crate::remote::download_dir(
                    &self.display,
                    &crate::remote::TransferOptions::default(),
                ))?;
                return crate::remote::cached_path(&self.display).map(PathBuf::from);
            }
            return futures::executor::block_on(crate::remote::ensure_downloaded(
                &self.display,
                &crate::remote::TransferOptions::default(),
            ))
            .map(PathBuf::from);
        }
        crate::plugin_vfs::materialize(&self.display).map(PathBuf::from)
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
        let session_id = crate::mounts::new_id().unwrap_or_default();
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
