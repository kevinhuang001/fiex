use super::*;
/// Create an empty file or a directory inside `path`, returning its URI.
pub fn create(path: &str, name: &str) -> Result<String, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let created = Some(mount.mkdir(&node_id, name)?);
    match created {
        Some(entry) => {
            let relative = child(&uri.relative, &entry.name)?;
            crate::plugins::mounts::bind_node(&uri.mount_id, &relative, &entry.node_id)?;
            crate::plugins::mounts::uri(&uri.mount_id, &relative)
        }
        None => crate::plugins::mounts::uri(&uri.mount_id, &child(&uri.relative, name)?),
    }
}
/// Create an empty file inside `path`, returning its URI. This uses the
/// provider's write stream with a parent node and name, so it is available
/// exactly when the provider implements `write` for that directory.
pub fn create_file(path: &str, name: &str) -> Result<String, String> {
    crate::plugins::mounts::segment(name)?;
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let entry = mount
        .begin_write(WriteRequest {
            node_id: None,
            parent_id: Some(&node_id),
            name: Some(name),
            size: Some(0),
            if_revision: None,
        })?
        .commit()?;
    let relative = child(&uri.relative, name)?;
    if let Some(entry) = entry {
        crate::plugins::mounts::bind_node(&uri.mount_id, &relative, &entry.node_id)?;
    }
    crate::plugins::mounts::uri(&uri.mount_id, &relative)
}
/// Rename the node at `path`, returning its new URI.
pub fn rename(path: &str, name: &str) -> Result<String, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let entry = mount.rename(&node_id, name, None)?;
    let parent = uri
        .relative
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let relative = child(parent, &entry.name)?;
    crate::plugins::mounts::bind_node(&uri.mount_id, &relative, &entry.node_id)?;
    crate::plugins::mounts::uri(&uri.mount_id, &relative)
}

pub fn remove(path: &str) -> Result<(), String> {
    let (uri, view, node_id) = node(path)?;
    mount_for(&uri, &view).remove(&node_id)
}
