//! Desktop invocations capture their own selected resources. Plugin callers
//! cannot reach this constructor: they receive only refs bound to their event.
use super::*;
use std::path::Path;

pub(crate) fn trusted(path: &str, owner: &str) -> Result<ResourceBroker, String> {
    let snapshot = crate::plugins::resource::resource_context::ResourceSnapshot::resolve(path)?;
    let virtual_storage = crate::plugins::mounts::handles(path)
        || crate::usbfs::route(path).is_some()
        || crate::core::volumes::virtual_resolvers::view_for(path).is_some();
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
    ResourceBroker::resolve(&scope, owner, &target["ref"]).map_err(String::from)
}

/// Desktop file operations enter through the same adapter surface as plugin
/// calls. None of these helpers branch on local/USB/plugin/builtin storage.
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

pub(crate) fn rename_path(path: &str, name: &str, owner: &str) -> Result<String, String> {
    if !crate::plugins::mounts::handles(path)
        && crate::usbfs::route(path).is_none()
        && !crate::core::volumes::virtual_resolvers::opaque(path)
    {
        return local_rename(path, name, owner);
    }
    let source = trusted(path, owner)?;
    let parent = parent_path(path)?;
    let destination = child(&trusted(&parent, owner)?, name)?;
    source.rename(&destination)?;
    Ok(destination.path)
}

/// A local rename needs one directory capability that authorizes both the
/// source and the new sibling name. The scope covers exactly those two paths.
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
        crate::plugins::resource::resource_context::ResourceSnapshot::new(path, source_path);
    let target = serde_json::json!({
        "ref": snapshot.reference,
        "metadata": snapshot.metadata(),
    });
    scope.bind_resource(&target, Some(source_path))?;
    let source = ResourceBroker::resolve(&scope, owner, &target["ref"])?;
    let destination_display = destination_path.to_string_lossy().into_owned();
    let destination_snapshot =
        crate::plugins::resource::resource_context::ResourceSnapshot::from_metadata(
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
    source.rename(&destination)?;
    Ok(destination.path)
}

#[cfg(feature = "desktop")]
pub(crate) fn move_to_trash(path: &str, owner: &str) -> Result<(), String> {
    trusted(path, owner)?.move_to_trash().map_err(String::from)
}

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
    ResourceBroker::resolve(&parent.scope, &parent.owner, &issued["ref"]).map_err(String::from)
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
