use crate::core::fs::ResourceBroker;
use crate::plugin_sdk::filesystem::PreflightPhase;
use fiex_provider_contracts::TransferKind;

pub(super) struct Step {
    pub(super) source: ResourceBroker,
    pub(super) target: ResourceBroker,
    pub(super) policy_parent: ResourceBroker,
    pub(super) action: TransferKind,
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
        let relative_name =
            super::planner::relative_target_name(&self.policy_parent, &self.target, name);
        if same_mount && self.action == TransferKind::Move {
            self.source.review_move(
                &self.policy_parent,
                Some(&relative_name),
                PreflightPhase::Execute,
            )?;
        } else {
            self.source
                .review_export(self.action, PreflightPhase::Execute)?;
            self.policy_parent.review_import(
                self.action,
                &relative_name,
                &metadata,
                PreflightPhase::Execute,
            )?;
        }
        Ok(())
    }
}
