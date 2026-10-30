//! Explicit output directories for Feature actions, including virtual stores.
use super::*;
use crate::plugins::{
    resource::fs_access::Scope,
    security::policy::{Capability, PermissionPolicy},
};

pub(super) fn bind(
    params: &mut Value,
    scope: &mut Scope,
    policy: &PermissionPolicy<'_>,
    method: &str,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
) -> Result<(), String> {
    if !matches!(method, "hooks.invoke" | "contextMenu.execute")
        || !["files.create", "folders.create"]
            .iter()
            .any(|key| policy.declared().contains(*key))
    {
        return Ok(());
    }
    let inferred = if directory.is_none() && resources.len() == 1 {
        crate::core::fs::parent_path(&resources[0].display).ok()
    } else {
        None
    };
    let Some(path) = directory.or(inferred.as_deref()) else {
        return Ok(());
    };
    let snapshot = ResourceSnapshot::resolve(path)?;
    if !snapshot.is_dir {
        return Err("action output must be a directory".into());
    }
    // An explicit destination is independent of source IO. Never add it to
    // targets/selection or give it the source node's mutation permissions.
    let directory = json!(snapshot.project("", "", policy.allowed(Capability::Metadata), false));
    if crate::plugins::mounts::handles(path)
        || crate::usbfs::route(path).is_some()
        || crate::core::volumes::virtual_resolvers::view_for(path).is_some()
    {
        scope.bind_remote(&directory, &snapshot)?;
    } else {
        scope.bind_resource(&directory, Some(std::path::Path::new(path)))?;
    }
    params["directory"] = directory;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_virtual_output_is_bound_without_becoming_a_selected_target() {
        let dir = tempfile::tempdir().unwrap();
        let mut scope = Scope::selected(dir.path(), &[]).unwrap();
        let declared = ["files.create".to_owned()].into_iter().collect();
        let policy = PermissionPolicy::new("org.example.archive", &declared);
        let source = ResourceSnapshot::from_metadata("source.txt", Default::default());
        let mut params = json!({"targets":[{"ref":source.reference}]});
        bind(
            &mut params,
            &mut scope,
            &policy,
            "contextMenu.execute",
            &[source],
            Some(crate::core::volumes::applications::URI),
        )
        .unwrap();
        assert_eq!(params["targets"].as_array().unwrap().len(), 1);
        let parent = crate::core::fs::ResourceBroker::resolve(
            &scope,
            "core:test-destination",
            &params["directory"]["ref"],
        )
        .unwrap();
        let child = crate::core::fs::child(&parent, "new-child.txt").unwrap();
        assert_eq!(child.reference.mount_id, "core:applications");
        assert!(child.path.starts_with("applications://~"));
        assert!(scope
            .authorize(&std::env::temp_dir().to_string_lossy())
            .is_err());
    }
}
