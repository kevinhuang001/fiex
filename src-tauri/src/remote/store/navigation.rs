use super::*;
use fiex_provider_contracts::ConnectionOpenResult;
/// Resolve a saved Connection via its owning plugin. The returned destination
/// can only be a registered VFS mount or vetted HTML from the plugin package.
pub fn open_navigation(id: &str) -> Result<serde_json::Value, String> {
    let connection = load_stored()
        .into_iter()
        .find(|connection| connection.id == id)
        .ok_or("connection not found")?;
    let plugin_id = ensure_provider_access(&connection)?;
    let target = crate::plugins::resolvers::connection::invoke(
        &plugin_id,
        &connection.kind,
        &connection.mount_id,
        "connection.open",
        serde_json::json!({
            "connectionId": connection.id,
            "mountId": connection.mount_id,
            "interactionId": crate::plugins::mounts::new_id()?,
            "connection": provider_config(&connection),
        }),
    )?;
    let target: ConnectionOpenResult =
        serde_json::from_value(target).map_err(|error| format!("invalid connection navigation: {error}"))?;
    match target {
        ConnectionOpenResult::Vfs { mount_id, node_ref } => {
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
            if node_ref.mount_id != connection.mount_id {
                return Err("PermissionDenied: foreign nodeRef mount".into());
            }
            Ok(serde_json::json!({
                "kind": "vfs",
                "path": crate::plugins::mounts::uri(&connection.mount_id, &node_ref.node_id)?,
            }))
        }
        ConnectionOpenResult::PluginView {
            view,
            state,
            tab_title,
        } => {
            if view.source != "package" {
                return Err("pluginView source must be its plugin package".into());
            }
            let tab_title = tab_title
                .as_deref()
                .map(str::trim)
                .filter(|value| {
                    !value.is_empty()
                        && value.chars().count() <= 80
                        && !value.chars().any(char::is_control)
                })
                .map(str::to_owned);
            if serde_json::to_vec(&state)
                .map_err(|error| error.to_string())?
                .len()
                > 64 * 1024
            {
                return Err("pluginView state exceeds size limit".into());
            }
            let asset_path = crate::plugins::host::plugin_view_asset(&plugin_id, &view.entry)?;
            Ok(serde_json::json!({
                "kind": "pluginView",
                "pluginId": plugin_id,
                "connectionId": id,
                "assetPath": asset_path,
                "state": state,
                "tabTitle": tab_title,
            }))
        }
    }
}
