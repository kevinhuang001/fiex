//! Project Core-owned resource identities at the host boundary.
//! Business parameters are opaque; this module never rewrites path-like strings.
use crate::{
    plugins::resource::fs_access::Scope, plugins::resource::resource_context::ResourceSnapshot,
};
use serde_json::{json, Value};
pub fn project(
    plugin: &str,
    params: &mut Value,
    scope: &mut Scope,
    metadata: bool,
) -> Result<(), String> {
    if !params.is_object() {
        return Ok(());
    }
    let paths = scope.selected_paths();
    if params.get("targets").is_none() {
        let mut targets = Vec::new();
        for path in &paths {
            let resource = ResourceSnapshot::new(&path.to_string_lossy(), path);
            let target = json!(resource.project(plugin, "", metadata, false));
            scope.bind_resource(&target, Some(path))?;
            targets.push(target);
        }
        params["targets"] = json!(targets);
    }
    let root = params["directory"]
        .as_str()
        .map(std::path::PathBuf::from)
        .filter(|path| scope.authorize(&path.to_string_lossy()).is_ok())
        .unwrap_or_else(|| scope.path().to_path_buf());
    if scope.authorize(&root.to_string_lossy()).is_ok() {
        let resource = ResourceSnapshot::new(&root.to_string_lossy(), &root);
        let directory = json!(resource.project(plugin, "", metadata, false));
        scope.bind_resource(&directory, Some(&root))?;
        params["directory"] = directory;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_preserves_business_fields_and_binds_only_the_selection() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected.txt");
        std::fs::write(&selected, "content").unwrap();
        let mut scope = Scope::selected(root.path(), std::slice::from_ref(&selected)).unwrap();
        let mut params =
            json!({"source":"selected.txt","path":"selected.txt","root":"custom-root"});
        project("org.test", &mut params, &mut scope, false).unwrap();
        assert_eq!(params["source"], "selected.txt");
        assert_eq!(params["path"], "selected.txt");
        assert_eq!(params["root"], "custom-root");
        assert!(params["targets"][0].get("metadata").is_none());
        assert!(scope
            .resolve_reference(&params["targets"][0]["ref"])
            .is_ok());
        assert!(scope
            .authorize(&root.path().join("other.txt").to_string_lossy())
            .is_err());
    }
}
