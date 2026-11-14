//! Scoped filesystem resource handle and provider operation façade.

use super::{FsError, FsErrorKind, ReadHandle, mapped_access, provider_registry};
use crate::plugins::resource::fs_access::Scope;
use fiex_provider_contracts::{FsOperation, ResourceRef};
use serde_json::Value;

/// This value can only be constructed from a ref captured by Core's Scope.
pub struct ResourceBroker {
    pub reference: ResourceRef,
    pub(crate) path: String,
    pub(crate) scope: Scope,
    pub(crate) storage_scope: Scope,
    pub(crate) owner: String,
    user_action: Option<mapped_access::UserActionContext>,
    mapped_operation: Option<FsOperation>,
}

mod reads;
mod writes;

impl ResourceBroker {
    pub fn resolve(scope: &Scope, owner: &str, reference: &Value) -> Result<Self, FsError> {
        if !scope.resource_bound(reference)? {
            return Err("ScopeDenied: resource not authorized in this operation".into());
        }
        let reference_typed = serde_json::from_value(reference.clone())
            .map_err(|e| format!("invalid ResourceRef: {e}"))?;
        let path = scope
            .remote_target(&serde_json::json!({"ref":reference}))?
            .map(|snapshot| snapshot.display)
            .map(Ok)
            .unwrap_or_else(|| scope.resolve_reference(reference))?;
        let (storage_scope, mapped_operation) = scope
            .mapped_storage_scope(reference, &path)?
            .unwrap_or_else(|| (scope.clone(), None));
        let path = if mapped_operation.is_some() {
            storage_scope.resolve_reference(reference).unwrap_or(path)
        } else {
            path
        };
        Ok(Self {
            reference: reference_typed,
            path,
            user_action: scope.user_action_context(),
            storage_scope,
            scope: scope.clone(),
            owner: owner.into(),
            mapped_operation,
        })
    }

    pub(crate) fn with_user_action_context(
        mut self,
        user_action: mapped_access::UserActionContext,
    ) -> Self {
        self.user_action = Some(user_action);
        self
    }

    pub(crate) fn user_action_context(&self) -> Option<&mapped_access::UserActionContext> {
        self.user_action.as_ref()
    }
    pub(crate) fn open_read_handle(&self) -> Result<Box<dyn ReadHandle>, FsError> {
        self.require(FsOperation::Read, false)?;
        provider_registry::open_read(self)
    }
    pub fn require(&self, operation: FsOperation, create: bool) -> Result<(), FsError> {
        if let Some(granted) = self.mapped_operation {
            let allowed = granted == operation || operation == FsOperation::Stat;
            if !allowed {
                return Err(FsError::new(
                    FsErrorKind::PermissionDenied,
                    format!(
                        "mapped resource grants {}, not {}",
                        granted.as_str(),
                        operation.as_str()
                    ),
                ));
            }
        }
        provider_registry::require(self, operation, create)
    }
    pub(crate) fn clone_binding(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            path: self.path.clone(),
            scope: self.scope.clone(),
            storage_scope: self.storage_scope.clone(),
            owner: self.owner.clone(),
            user_action: self.user_action,
            mapped_operation: self.mapped_operation,
        }
    }
}
