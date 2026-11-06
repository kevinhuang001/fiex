//! Desktop invocations capture their own selected resources. Plugin callers
//! cannot reach this constructor: they receive only refs bound to their event.
use super::*;
use std::path::Path;

pub(crate) fn trusted(path: &str, owner: &str) -> Result<ResourceBroker, String> {
    let snapshot = crate::plugins::resource::resource_context::resolve(path)?;
    trusted_snapshot(&snapshot, owner)
}

pub(crate) fn trusted_with_user_action(
    path: &str,
    owner: &str,
    user_action: super::mapped_access::UserActionContext,
) -> Result<ResourceBroker, String> {
    Ok(trusted(path, owner)?.with_user_action_context(user_action))
}

/// Bind a Core-created snapshot to a fresh operation scope without resolving
/// provider metadata a second time.
pub(crate) fn trusted_snapshot(
    snapshot: &crate::plugins::resource::resource_context::ResourceSnapshot,
    owner: &str,
) -> Result<ResourceBroker, String> {
    let path = snapshot.display.as_str();
    let provider_owned = crate::core::fs::provider_registry::resource_access(path)
        .map_err(String::from)?
        == crate::core::fs::provider_registry::ResourceAccess::Provider;
    let mut scope = if provider_owned {
        Scope::selected(&std::env::temp_dir(), &[])?
    } else {
        let local = Path::new(path);
        Scope::selected(local.parent().unwrap_or(local), &[local.to_path_buf()])?
    };
    let target = serde_json::json!({"ref":&snapshot.reference,"metadata":snapshot.metadata()});
    if provider_owned {
        scope.bind_remote(&target, snapshot)?;
    } else {
        scope.bind_resource(&target, Some(Path::new(path)))?;
    }
    ResourceBroker::resolve(&scope, owner, &target["ref"]).map_err(String::from)
}

/// Desktop file operations enter through the same provider surface as plugin
/// calls. Storage selection stays inside the VFS registry.
#[cfg(any(feature = "desktop", test))]
pub(crate) fn create(
    path: &str,
    name: &str,
    directory: bool,
    owner: &str,
) -> Result<String, String> {
    trusted(path, owner)?
        .create(name, directory)
        .map_err(String::from)
}

#[cfg(any(feature = "desktop", test))]
pub(crate) fn rename_path(path: &str, name: &str, owner: &str) -> Result<String, String> {
    if crate::core::fs::provider_registry::resource_access(path).map_err(String::from)?
        == crate::core::fs::provider_registry::ResourceAccess::HostPath
    {
        return local_rename(path, name, owner);
    }
    let source = trusted(path, owner)?;
    let parent = parent_path(path)?;
    let kind = if source.is_dir()? {
        crate::core::fs::provider_registry::ChildKind::Folder
    } else {
        crate::core::fs::provider_registry::ChildKind::File
    };
    let destination = child(&trusted(&parent, owner)?, name, kind)?;
    source.rename(&destination).map_err(String::from)
}

/// A local rename needs one directory capability that authorizes both the
/// source and the new sibling name. The scope covers exactly those two paths.
#[cfg(any(feature = "desktop", test))]
fn local_rename(path: &str, name: &str, owner: &str) -> Result<String, String> {
    crate::plugins::mounts::segment(name)?;
    let source_path = Path::new(path);
    let parent = source_path.parent().ok_or("item has no parent")?;
    let destination_path = parent.join(name);
    let mut scope = Scope::selected(
        parent,
        &[source_path.to_path_buf(), destination_path.clone()],
    )?;
    let snapshot =
        crate::plugins::resource::resource_context::new_resource_snapshot(path, source_path);
    let target = serde_json::json!({
        "ref": snapshot.reference,
        "metadata": snapshot.metadata(),
    });
    scope.bind_resource(&target, Some(source_path))?;
    let source = ResourceBroker::resolve(&scope, owner, &target["ref"])?;
    let destination_display = destination_path.to_string_lossy().into_owned();
    let destination_snapshot = crate::plugins::resource::resource_context::from_metadata(
        &destination_display,
        crate::plugins::contracts::ResourceMetadata {
            name: name.into(),
            kind: "file".into(),
            ..Default::default()
        },
    );
    let destination_target = serde_json::json!({
        "ref": destination_snapshot.reference,
        "metadata": destination_snapshot.metadata(),
    });
    scope.bind_resource(&destination_target, Some(&destination_path))?;
    let destination = ResourceBroker::resolve(&scope, owner, &destination_target["ref"])?;
    source.rename(&destination).map_err(String::from)
}

#[cfg(feature = "desktop")]
pub(crate) fn move_to_trash(path: &str, owner: &str) -> Result<(), String> {
    trusted(path, owner)?.move_to_trash().map_err(String::from)
}

#[cfg(test)]
pub(crate) fn remove_path(path: &str, owner: &str) -> Result<(), String> {
    trusted(path, owner)?.remove().map_err(String::from)
}

/// Bind a selected destination directory, including a provider root. Child
/// refs are minted only beneath that directory within the same scope.
pub(crate) fn destination(path: &str, owner: &str) -> Result<ResourceBroker, String> {
    let directory = trusted(path, owner)?;
    if !directory.is_dir()? {
        return Err("destination is not a directory".into());
    }
    Ok(directory)
}

pub(crate) fn child(
    parent: &ResourceBroker,
    name: &str,
    kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<ResourceBroker, String> {
    let path = crate::core::fs::provider_registry::child_path(&parent.path, name, kind)?;
    let parent_ref = serde_json::to_value(&parent.reference).map_err(|e| e.to_string())?;
    let issued = parent.scope.issue_child(&parent_ref, &path, None)?;
    ResourceBroker::resolve(&parent.scope, &parent.owner, &issued["ref"]).map_err(String::from)
}

/// Navigation-to-parent resolution stays in Core. Features receive only the
/// selected directory ref and do not inspect schemes or provider identities.
pub(crate) fn parent_path(path: &str) -> Result<String, String> {
    crate::core::fs::provider_registry::parent_path(path)
}
