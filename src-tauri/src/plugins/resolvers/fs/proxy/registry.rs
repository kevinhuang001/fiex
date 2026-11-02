use super::IpcVfsProvider;
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderInstances, ProviderKind, ProviderMatchRule, VfsProvider,
};
use std::sync::{Arc, Mutex, OnceLock};

fn registry() -> &'static Mutex<ProviderInstances<dyn VfsProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn VfsProvider>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Vfs)))
}

pub fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

pub(super) fn provider(plugin_id: &str) -> Result<Arc<dyn VfsProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing VfsProvider plugin identity".into());
    }
    let mut registry = registry()
        .lock()
        .map_err(|_| "VfsProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(provider) = registry.get_arc(&id) {
        return Ok(provider);
    }
    registry.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Vfs,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcVfsProvider {
            plugin_id: plugin_id.into(),
        }),
    )?;
    registry
        .get_arc(&id)
        .ok_or_else(|| "VfsProvider registration failed".into())
}
