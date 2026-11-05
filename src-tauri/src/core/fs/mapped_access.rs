//! Resolve persisted resource mappings without persisting operation scopes.
use super::{FsOperation, ResourceBroker};
use crate::plugins::resource::{fs_access::Scope, resource_context::ResourceSnapshot};

/// Constructed only at trusted Core event or desktop command boundaries.
/// Provider payloads and persisted mapping data cannot create this token.
#[derive(Clone, Copy)]
pub struct UserActionContext {
    _private: (),
}

impl UserActionContext {
    pub(crate) fn from_core_event() -> Self {
        Self { _private: () }
    }

    #[cfg(feature = "desktop")]
    pub(crate) fn from_desktop_command() -> Self {
        Self { _private: () }
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self { _private: () }
    }
}

/// The current resource's operation lifetime and optional trusted UI action.
/// Mapping data itself never carries a scope or an authorization decision.
pub(crate) struct OperationScope<'a> {
    current: &'a ResourceBroker,
    user_action: Option<UserActionContext>,
}

impl<'a> OperationScope<'a> {
    pub(crate) fn new(current: &'a ResourceBroker) -> Self {
        Self {
            current,
            user_action: current.user_action_context().copied(),
        }
    }
}

/// Resolve and authorize a persisted locator for one requested operation.
/// The locator is identity data only; an existing bound scope or an explicit
/// Core-owned user action is required before Core resolves the source.
pub(crate) fn authorize_locator(
    locator: &str,
    current_scope: &Scope,
    owner: &str,
    user_action: Option<UserActionContext>,
    operation: FsOperation,
) -> Result<(ResourceBroker, ResourceSnapshot), String> {
    let bound = current_scope
        .reference_for_path(locator)
        .ok()
        .and_then(|reference| current_scope.resource_bound(&reference).ok())
        .unwrap_or(false);
    let user_action = user_action.or_else(|| current_scope.user_action_context());
    if !bound && user_action.is_none() {
        return Err("NeedsAuthorization: mapped resource requires a user action".into());
    }

    let snapshot = crate::plugins::resource::resource_context::resolve(locator)?;
    authorize_snapshot(snapshot, current_scope, owner, user_action, operation)
}

pub(crate) fn authorize_snapshot(
    snapshot: ResourceSnapshot,
    current_scope: &Scope,
    owner: &str,
    user_action: Option<UserActionContext>,
    operation: FsOperation,
) -> Result<(ResourceBroker, ResourceSnapshot), String> {
    let bound = current_scope
        .reference_for_path(&snapshot.display)
        .ok()
        .and_then(|reference| current_scope.resource_bound(&reference).ok())
        .unwrap_or(false);
    let user_action = user_action.or_else(|| current_scope.user_action_context());
    if !bound && user_action.is_none() {
        return Err("NeedsAuthorization: mapped resource requires a user action".into());
    }
    let access = crate::core::fs::provider_registry::resource_access(&snapshot.display)
        .map_err(String::from)?;
    if access == crate::core::fs::provider_registry::ResourceAccess::HostPath && !bound {
        if user_action.is_none() {
            return Err("NeedsAuthorization: mapped resource requires a user action".into());
        }
        #[cfg(feature = "desktop")]
        {
            let selected = if snapshot.is_dir {
                rfd::FileDialog::new().pick_folder()
            } else {
                rfd::FileDialog::new().pick_file()
            }
            .ok_or_else(|| "NeedsAuthorization: file authorization was cancelled".to_string())?;
            let expected =
                dunce::canonicalize(&snapshot.local).map_err(|error| error.to_string())?;
            let selected = dunce::canonicalize(selected).map_err(|error| error.to_string())?;
            if expected != selected {
                return Err("NeedsAuthorization: selected source does not match mapping".into());
            }
        }
        #[cfg(not(feature = "desktop"))]
        {
            return Err("NeedsAuthorization: local mapped access requires desktop consent".into());
        }
    }

    let mut broker = crate::core::fs::trusted_snapshot(&snapshot, owner)?;
    if let Some(user_action) = user_action {
        broker = broker.with_user_action_context(user_action);
    }
    broker.require(operation, false).map_err(String::from)?;
    Ok((broker, snapshot))
}

/// Recent and Plugin VFS mapping reads use the same operation-specific path.
pub(crate) fn authorize_source(
    locator: &str,
    scope: &OperationScope<'_>,
    operation: FsOperation,
) -> Result<ResourceBroker, String> {
    let (broker, _) = authorize_locator(
        locator,
        &scope.current.scope,
        &scope.current.owner,
        scope.user_action,
        operation,
    )?;
    Ok(broker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::resource::fs_access::Scope;

    fn target(snapshot: &ResourceSnapshot) -> serde_json::Value {
        serde_json::json!({"ref":snapshot.reference,"metadata":snapshot.metadata()})
    }

    #[test]
    fn a_locator_is_not_resolved_without_a_bound_scope_or_user_action() {
        let directory = tempfile::tempdir().unwrap();
        let scope = Scope::selected(directory.path(), &[]).unwrap();
        let error = authorize_locator(
            "this is not a valid resource locator",
            &scope,
            "plugin:test",
            None,
            FsOperation::Read,
        )
        .err()
        .expect("an untrusted locator must require authorization");
        assert!(error.starts_with("NeedsAuthorization:"));
    }

    #[test]
    fn delegated_mapped_refs_are_limited_to_the_granted_operation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mapped.txt");
        std::fs::write(&path, b"mapped").unwrap();
        let snapshot = crate::plugins::resource::resource_context::new_resource_snapshot(
            &path.to_string_lossy(),
            &path,
        );
        let scope = Scope::selected(directory.path(), &[]).unwrap();
        scope
            .bind_mapped_resource(snapshot.clone(), FsOperation::Read)
            .unwrap();
        let reference = serde_json::to_value(&snapshot.reference).unwrap();
        let broker = ResourceBroker::resolve(&scope, "plugin:test", &reference).unwrap();
        assert!(broker.require(FsOperation::Read, false).is_ok());
        assert!(broker.require(FsOperation::Stat, false).is_ok());
        assert!(broker.require(FsOperation::Write, false).is_err());
        let metadata = broker.stat().unwrap();
        assert_eq!(metadata["name"], "mapped.txt");
        let mut reader = broker.open_read_handle().unwrap();
        let (bytes, eof) = reader.read_at(0, 16).unwrap();
        assert_eq!(bytes, b"mapped");
        assert!(eof);
    }

    #[test]
    fn a_mapped_directory_scope_can_list_and_resolve_its_children() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("mapped-folder");
        std::fs::create_dir(&root).unwrap();
        let path = root.join("child.txt");
        std::fs::write(&path, b"child").unwrap();
        let snapshot = crate::plugins::resource::resource_context::new_resource_snapshot(
            &root.to_string_lossy(),
            &root,
        );
        let scope = Scope::selected(directory.path(), &[]).unwrap();
        scope
            .bind_mapped_resource(snapshot.clone(), FsOperation::List)
            .unwrap();
        let reference = serde_json::to_value(&snapshot.reference).unwrap();
        let broker = ResourceBroker::resolve(&scope, "plugin:test", &reference).unwrap();
        let listing = broker.list().unwrap();
        let child_path = listing[0]["path"].as_str().unwrap();
        let child_reference = scope.issue_child(&reference, child_path, None).unwrap();
        let child =
            ResourceBroker::resolve(&scope, "plugin:test", &child_reference["ref"]).unwrap();
        let mut reader = child.open_read_handle().unwrap();
        let (bytes, eof) = reader.read_at(0, 16).unwrap();
        assert_eq!(bytes, b"child");
        assert!(eof);
        assert_eq!(listing.as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_background_task_does_not_inherit_a_mapped_action_grant() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mapped.txt");
        std::fs::write(&path, b"mapped").unwrap();
        let snapshot = crate::plugins::resource::resource_context::new_resource_snapshot(
            &path.to_string_lossy(),
            &path,
        );
        let mut scope = Scope::selected(directory.path(), &[]).unwrap();
        scope.set_user_action_context(UserActionContext::for_test());
        scope
            .bind_mapped_resource(snapshot.clone(), FsOperation::Read)
            .unwrap();
        let reference = serde_json::to_value(&snapshot.reference).unwrap();
        let task = scope.without_user_action();
        assert!(!task.resource_bound(&reference).unwrap());
    }

    #[test]
    fn background_mapped_access_never_requests_interactive_authorization() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("mapped.txt");
        std::fs::write(&source, b"mapped").unwrap();
        let snapshot = crate::plugins::resource::resource_context::from_metadata(
            "recent://test",
            crate::plugins::contracts::ResourceMetadata {
                kind: "file".into(),
                ..Default::default()
            },
        );
        let mut resource_scope = Scope::selected(directory.path(), &[]).unwrap();
        resource_scope
            .bind_remote(&target(&snapshot), &snapshot)
            .unwrap();
        let current =
            ResourceBroker::resolve(&resource_scope, "core:recent", &target(&snapshot)["ref"])
                .unwrap();
        let scope = OperationScope::new(&current);
        let error = authorize_source(&source.to_string_lossy(), &scope, FsOperation::Read)
            .err()
            .expect("background mapped access must be denied");
        assert!(error.starts_with("NeedsAuthorization:"));
    }
}
