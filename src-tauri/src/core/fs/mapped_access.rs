//! Resolve persisted resource mappings without persisting operation scopes.
use super::{FsOperation, ResourceBroker};

/// Constructed only at trusted desktop command boundaries. Provider calls and
/// background discovery never receive this interaction capability.
#[derive(Clone, Copy)]
pub struct UserActionContext {
    _private: (),
}

impl UserActionContext {
    #[cfg(any(feature = "desktop", test))]
    pub(crate) fn from_desktop_command() -> Self {
        Self { _private: () }
    }
}

/// The current resource's operation lifetime and optional trusted UI action.
/// Mapping data itself never carries a scope or an authorization decision.
pub(crate) struct OperationScope<'a> {
    current: &'a ResourceBroker,
    user_action: Option<&'a UserActionContext>,
}

impl<'a> OperationScope<'a> {
    pub(crate) fn new(current: &'a ResourceBroker) -> Self {
        Self {
            current,
            user_action: current.user_action_context(),
        }
    }
}

/// Resolve an untrusted persisted locator against the current operation. A
/// local host-path mapping may prompt only when the current scope was created
/// by a trusted desktop user action; background work returns NeedsAuthorization.
pub(crate) fn authorize_source(
    locator: &str,
    scope: &OperationScope<'_>,
    operation: FsOperation,
) -> Result<ResourceBroker, String> {
    if crate::core::fs::provider_registry::resource_access(locator).map_err(String::from)?
        == crate::core::fs::provider_registry::ResourceAccess::HostPath
    {
        if scope.user_action.is_none() {
            return Err("NeedsAuthorization: mapped resource requires a user action".into());
        }
        #[cfg(feature = "desktop")]
        {
            let Some(selected) = rfd::FileDialog::new().pick_file() else {
                return Err("NeedsAuthorization: file authorization was cancelled".into());
            };
            let expected = dunce::canonicalize(locator).map_err(|error| error.to_string())?;
            let selected = dunce::canonicalize(selected).map_err(|error| error.to_string())?;
            if expected != selected {
                return Err(
                    "NeedsAuthorization: selected file does not match mapped source".into(),
                );
            }
        }
        #[cfg(not(feature = "desktop"))]
        {
            return Err(
                "NeedsAuthorization: local mapped access needs a user file selection".into(),
            );
        }
    }
    let broker = super::trusted(locator, &scope.current.owner)?;
    broker.require(operation, false).map_err(String::from)?;
    Ok(broker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::resource::fs_access::Scope;

    #[test]
    fn background_mapped_access_never_requests_interactive_authorization() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("mapped.txt");
        std::fs::write(&source, b"mapped").unwrap();
        let snapshot = crate::plugins::resource::resource_context::ResourceSnapshot::from_metadata(
            "recent://test",
            crate::plugins::contracts::ResourceMetadata {
                kind: "file".into(),
                ..Default::default()
            },
        );
        let target = serde_json::json!({"ref":snapshot.reference,"metadata":snapshot.metadata()});
        let mut resource_scope = Scope::selected(directory.path(), &[]).unwrap();
        resource_scope.bind_remote(&target, &snapshot).unwrap();
        let current =
            ResourceBroker::resolve(&resource_scope, "core:recent", &target["ref"]).unwrap();
        let scope = OperationScope::new(&current);
        let error = authorize_source(&source.to_string_lossy(), &scope, FsOperation::Read)
            .err()
            .expect("background mapped access must be denied");
        assert!(error.starts_with("NeedsAuthorization:"));
    }
}
