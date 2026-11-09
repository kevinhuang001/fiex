//! Resource metadata and operation-specific preflight policy.
use crate::core::fs::{FsError, FsErrorKind, ResourceBroker, binding, provider_registry};
use crate::plugin_sdk::filesystem::{
    CreateFilePreflight, CreateFolderPreflight, ExportPreflight, ImportPreflight, MovePreflight,
    PreflightOperation, PreflightPhase, PreflightRequest, RemovePreflight, RenamePreflight,
    TransferKind, TrashPreflight,
};
use fiex_provider_contracts::TransferMetadata;

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
        assert!(
            view.review_import(
                TransferKind::Copy,
                "document.txt",
                &metadata,
                PreflightPhase::Execute,
            )
            .is_err()
        );
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
