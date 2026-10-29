use super::*;
/// Resolve a saved Connection via its owning plugin. The returned destination
/// can only be a registered VFS mount or vetted HTML from the plugin package.
pub fn open_navigation(id: &str) -> Result<serde_json::Value, String> {
    let connection = load_stored()
        .into_iter()
        .find(|connection| connection.id == id)
        .ok_or("connection not found")?;
    let plugin_id = ensure_provider_access(&connection)?;
    let target = crate::plugins::events::invoke(
        &plugin_id,
        "connection.open",
        serde_json::json!({
            "connectionId": connection.id,
            "mountId": connection.mount_id,
            "interactionId": crate::plugins::mounts::new_id()?,
            "connection": provider_config(&connection),
        }),
    )?;
    match target.get("kind").and_then(serde_json::Value::as_str) {
        Some("vfs") => {
            let mount_id = target
                .get("mountId")
                .and_then(serde_json::Value::as_str)
                .ok_or("VFS navigation missing mountId")?;
            if mount_id != connection.mount_id {
                return Err("PermissionDenied: foreign connection mount".into());
            }
            let registered = crate::plugins::host::catalog()
                .into_iter()
                .find(|plugin| plugin.id == plugin_id)
                .and_then(|plugin| plugin.extensions.get("vfs").cloned())
                .is_some_and(|vfs| vfs.is_object());
            if !registered {
                return Err("PermissionDenied: VFS extension not registered".into());
            }
            let node_ref = target
                .get("nodeRef")
                .ok_or("VFS navigation missing nodeRef")?;
            if node_ref.get("mountId").and_then(serde_json::Value::as_str)
                != Some(connection.mount_id.as_str())
            {
                return Err("PermissionDenied: foreign nodeRef mount".into());
            }
            let node_id = node_ref
                .get("nodeId")
                .and_then(serde_json::Value::as_str)
                .ok_or("VFS navigation missing nodeId")?;
            Ok(serde_json::json!({
                "kind": "vfs",
                "path": crate::plugins::mounts::uri(&connection.mount_id, node_id)?,
            }))
        }
        Some("pluginView") => {
            let view = target.get("view").ok_or("pluginView missing view")?;
            if view.get("source").and_then(serde_json::Value::as_str) != Some("package") {
                return Err("pluginView source must be its plugin package".into());
            }
            let entry = view
                .get("entry")
                .and_then(serde_json::Value::as_str)
                .ok_or("pluginView missing package entry")?;
            let tab_title = target
                .get("tabTitle")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| {
                    !value.is_empty()
                        && value.chars().count() <= 80
                        && !value.chars().any(char::is_control)
                })
                .map(str::to_owned);
            let state = target
                .get("state")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if serde_json::to_vec(&state)
                .map_err(|error| error.to_string())?
                .len()
                > 64 * 1024
            {
                return Err("pluginView state exceeds size limit".into());
            }
            let asset_path = crate::plugins::host::plugin_view_asset(&plugin_id, entry)?;
            Ok(serde_json::json!({
                "kind": "pluginView",
                "pluginId": plugin_id,
                "connectionId": id,
                "assetPath": asset_path,
                "state": state,
                "tabTitle": tab_title,
            }))
        }
        _ => Err("Connection must return vfs or pluginView navigation".into()),
    }
}
