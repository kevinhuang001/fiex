//! Core-owned resource snapshot shared by Info and Preview providers.
//! Metadata is retrieved once, then projected separately for each provider.
//! The snapshot never crosses the plugin process boundary.
use crate::plugin_contracts::{
    EventContext, ResourceContext, ResourceMetadata, ResourceRef, ScopedRead,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct ResourceSnapshot {
    pub local: PathBuf,
    pub reference: ResourceRef,
    pub stat: Option<fs::Metadata>,
    pub is_dir: bool,
    pub ext: String,
    pub mime: String,
    pub name: String,
    metadata: Option<ResourceMetadata>,
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
            local: local.to_path_buf(),
            reference,
            stat,
            is_dir,
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

    pub fn project(
        &self,
        owner: &str,
        session_id: &str,
        metadata_allowed: bool,
        read_allowed: bool,
    ) -> ResourceContext {
        ResourceContext {
            reference: self.reference.clone(),
            metadata: metadata_allowed.then(|| self.metadata.clone()).flatten(),
            read: (read_allowed && !self.is_dir).then(|| ScopedRead {
                resource_id: crate::plugin_scoped_reads::register(owner, session_id, &self.name),
            }),
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
        assert!(without_grant.targets[0].read.is_none());
        assert_eq!(
            with_meta.targets[0].metadata.as_ref().unwrap().size,
            Some(3)
        );
        assert!(with_meta.targets[0].read.is_none());
    }
}
