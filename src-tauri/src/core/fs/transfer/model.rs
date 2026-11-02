use crate::core::fs::ResourceBroker;
use fiex_provider_contracts::{TransferAction, TransferDirection};

pub(super) struct Step {
    pub(super) source: ResourceBroker,
    pub(super) target: ResourceBroker,
    pub(super) policy_parent: ResourceBroker,
    pub(super) action: TransferAction,
    pub(super) directory: bool,
    pub(super) existing: bool,
    pub(super) source_version: serde_json::Value,
    pub(super) target_version: Option<serde_json::Value>,
}

impl Step {
    pub(super) fn review(&self) -> Result<(), String> {
        let metadata = self.source.transfer_metadata()?;
        let same_mount = self.source.reference.mount_id == self.target.reference.mount_id;
        let name = self
            .target
            .path
            .rsplit(['/', '\\'])
            .next()
            .ok_or("missing target name")?;
        self.source.review_transfer(
            TransferDirection::Export,
            self.action,
            name,
            &metadata,
            same_mount,
        )?;
        self.policy_parent.review_transfer(
            TransferDirection::Import,
            self.action,
            name,
            &metadata,
            same_mount,
        )?;
        Ok(())
    }
}
