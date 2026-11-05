use crate::plugins::contracts::ResourceRef;

/// Private Core-UI binding. Plugin iframe SDKs receive only `reference`; the
/// path travels out of band and must be inside this invocation's selection.
#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceBinding {
    #[serde(rename = "ref")]
    pub reference: ResourceRef,
    pub path: String,
}
pub fn bind_selected(
    scope: &mut crate::plugins::resource::fs_access::Scope,
    bindings: &[ResourceBinding],
) -> Result<(), String> {
    for binding in bindings {
        if crate::core::fs::provider_registry::resource_access(&binding.path)
            .map_err(String::from)?
            == crate::core::fs::provider_registry::ResourceAccess::Provider
        {
            let snapshot = crate::plugins::resource::resource_context::resolve(&binding.path)?;
            if snapshot.reference != binding.reference {
                return Err("ScopeDenied: resource reference does not match selected node".into());
            }
            scope.bind_remote(
                &serde_json::json!({"ref":binding.reference,"metadata":snapshot.metadata()}),
                &snapshot,
            )?;
            continue;
        }
        // An attacker-controlled resource binding is never promoted to a
        // local path merely because its scheme has no registered mount.
        if !crate::core::volumes::native_vfs::is_host_path(&binding.path) {
            return Err("ScopeDenied: unsupported resource owner".into());
        }
        let path = std::path::Path::new(&binding.path);
        scope.authorize(&binding.path)?;
        let snapshot =
            crate::plugins::resource::resource_context::new_resource_snapshot(&binding.path, path);
        if snapshot.reference != binding.reference {
            return Err("ScopeDenied: resource reference does not match selected node".into());
        }
        scope.bind_resource(
            &serde_json::json!({"ref":binding.reference,"metadata":snapshot.metadata()}),
            Some(path),
        )?;
    }
    Ok(())
}
