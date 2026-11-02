//! Transfer review is additional policy, never a replacement for capabilities.
use super::*;
use fiex_provider_contracts::{
    TransferAction, TransferDirection, TransferMetadata, TransferPreflight,
};

impl ResourceBroker {
    pub(crate) fn transfer_metadata(&self) -> Result<TransferMetadata, FsError> {
        let stat = self.stat()?;
        let entry = self.desktop_entry()?;
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
            system: crate::core::volumes::applications::is_system_app(&self.path),
        })
    }
    pub(crate) fn review_transfer(
        &self,
        direction: TransferDirection,
        action: TransferAction,
        name: &str,
        metadata: &TransferMetadata,
        same_mount: bool,
    ) -> Result<(), FsError> {
        // Provider review is a cross-VFS contract. A transfer inside the same
        // VFS already has Core capability checks and must not prompt or run
        // provider-specific policy again.
        if same_mount {
            return Ok(());
        }
        provider_registry::preflight(
            self,
            &TransferPreflight {
                node_id: self.reference.node_id.clone(),
                direction,
                action,
                name: name.into(),
                metadata: metadata.clone(),
                same_mount,
            },
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
    fn same_vfs_transfers_skip_provider_preflight() {
        let view = crate::core::fs::binding::trusted(
            crate::core::volumes::applications::URI,
            "core:test-preflight",
        )
        .unwrap();
        let metadata = view.transfer_metadata().unwrap();
        // Applications rejects document imports across a VFS boundary.
        assert!(view
            .review_transfer(
                TransferDirection::Import,
                TransferAction::Copy,
                "document.txt",
                &metadata,
                false,
            )
            .is_err());
        // The same review is a no-op inside one VFS.
        view.review_transfer(
            TransferDirection::Import,
            TransferAction::Copy,
            "document.txt",
            &metadata,
            true,
        )
        .unwrap();
    }

    #[test]
    fn metadata_unknown_is_not_a_proof_of_an_executable_shortcut() {
        assert_eq!(
            executable_shortcut("fiex://mount/example/shortcut.lnk", None),
            None
        );
        assert_eq!(executable_shortcut("/tmp/document.txt", None), Some(false));
    }
}
