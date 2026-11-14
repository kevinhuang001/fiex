//! Validates provider execution results before the menu action reaches the UI.
use super::Resolver;
use fiex_provider_contracts::{ContextMenuProvider, MenuExecuteResult};

fn validate_plugin_action(
    plugin: &crate::plugins::host::PluginInfo,
    result: &mut MenuExecuteResult,
) -> Result<(), String> {
    match result {
        MenuExecuteResult::CoreAction(_)
        | MenuExecuteResult::ConnectionCoreAction(_)
        | MenuExecuteResult::RemoteAction(_)
        | MenuExecuteResult::NativeAction(_) => {
            Err("plugin returned a reserved action kind".into())
        }
        MenuExecuteResult::Panel(panel) => {
            let entry = panel.entry.as_deref().ok_or("panel result missing entry")?;
            let asset = crate::plugins::host::plugin_view_asset(&plugin.id, entry)?;
            panel.entry = None;
            panel.asset_path = Some(asset);
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn execute_provider(
    resolver: &Resolver,
    implementation: &dyn ContextMenuProvider,
    invocation: &dyn fiex_provider_contracts::ProviderInvocation,
) -> Result<MenuExecuteResult, String> {
    let mut result = implementation
        .execute(invocation)
        .map_err(|error| error.to_string())?;
    if let Some(plugin) = resolver.plugin.as_deref() {
        validate_plugin_action(plugin, &mut result)?;
    }
    Ok(result)
}
