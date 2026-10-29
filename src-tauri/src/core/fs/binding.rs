//! Desktop invocations capture their own selected resources. Plugin callers
//! cannot reach this constructor: they receive only refs bound to their event.
use super::*;
use std::path::Path;

pub(crate) fn trusted(path: &str, owner: &str) -> Result<ResourceBroker, String> {
    let snapshot = crate::plugins::resource::resource_context::ResourceSnapshot::resolve(path)?;
    let virtual_storage = crate::plugins::mounts::handles(path)
        || crate::usbfs::route(path).is_some()
        || crate::core::volumes::virtual_resolvers::resolver_for(path).is_some();
    let mut scope = if virtual_storage {
        Scope::selected(&std::env::temp_dir(), &[])?
    } else {
        let local = Path::new(path);
        Scope::selected(local.parent().unwrap_or(local), &[local.to_path_buf()])?
    };
    let target = serde_json::json!({"ref":snapshot.reference,"metadata":snapshot.metadata()});
    if virtual_storage {
        scope.bind_remote(&target, &snapshot)?;
    } else {
        scope.bind_resource(&target, Some(Path::new(path)))?;
    }
    ResourceBroker::resolve(&scope, owner, &target["ref"])
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

pub(crate) fn child(parent: &ResourceBroker, name: &str) -> Result<ResourceBroker, String> {
    crate::plugins::mounts::segment(name)?;
    let path = if crate::plugins::mounts::handles(&parent.path) {
        let uri = crate::plugins::mounts::parse(&parent.path)?;
        crate::plugins::mounts::uri(
            &uri.mount_id,
            format!("{}/{}", uri.relative.trim_matches('/'), name).trim_start_matches('/'),
        )?
    } else if crate::usbfs::route(&parent.path).is_some() {
        format!("{}/{}", parent.path.trim_end_matches('/'), name)
    } else if parent.path == crate::core::volumes::applications::URI
        || crate::core::volumes::applications::is_user(&parent.path)
    {
        let backing = if parent.path == crate::core::volumes::applications::URI {
            crate::core::volumes::applications::stored_path(&parent.path)?
        } else {
            crate::core::volumes::applications::candidate_path(&parent.path)?
        };
        crate::core::volumes::applications::stored_uri(&backing.join(name))?
    } else {
        Path::new(&parent.path)
            .join(name)
            .to_string_lossy()
            .into_owned()
    };
    let parent_ref = serde_json::to_value(&parent.reference).map_err(|e| e.to_string())?;
    let issued = parent.scope.issue_child(&parent_ref, &path, None)?;
    ResourceBroker::resolve(&parent.scope, &parent.owner, &issued["ref"])
}

/// Navigation-to-parent resolution stays in Core. Features receive only the
/// selected directory ref and do not inspect schemes or provider identities.
pub(crate) fn parent_path(path: &str) -> Result<String, String> {
    if crate::plugins::mounts::handles(path) {
        let uri = crate::plugins::mounts::parse(path)?;
        if uri.relative.is_empty() {
            return Err("mount root has no parent".into());
        }
        let parent = uri
            .relative
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        return crate::plugins::mounts::uri(&uri.mount_id, parent);
    }
    if crate::core::volumes::applications::is_user(path) {
        let local = crate::core::volumes::applications::candidate_path(path)?;
        let parent = local.parent().ok_or("item has no parent")?;
        if parent
            == crate::core::volumes::applications::stored_path(
                crate::core::volumes::applications::URI,
            )?
        {
            return Ok(crate::core::volumes::applications::URI.into());
        }
        return crate::core::volumes::applications::stored_uri(parent);
    }
    if crate::core::volumes::virtual_resolvers::opaque(path) {
        return Err("view has no writable parent".into());
    }
    Path::new(path)
        .parent()
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or_else(|| "item has no parent".into())
}
