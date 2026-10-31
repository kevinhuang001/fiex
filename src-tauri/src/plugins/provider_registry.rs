//! ProviderRegistry: the capabilities a plugin declared during
//! `plugin.initialize`.
//!
//! Core never guesses what a provider can do. A plugin reports the operations
//! it really implements, Core stores that set per plugin, and every `vfs.*`
//! request is checked against it before it leaves the process boundary. A
//! plugin that supports `rename` for one account can still restrict a single
//! node through `allowedOps`; user grants and the Operation Scope are checked
//! independently by the permission layer.
mod event_candidates;
pub(crate) mod native;
mod vfs_instances;
pub use event_candidates::candidates;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Mutex, OnceLock},
};

/// The SDK and Core use the same operation vocabulary and wire mapping.
pub use fiex_provider_contracts::FsOperation;

/// One plugin's declaration. `reported` distinguishes "declared nothing" from
/// "never handshook": only the former is an authoritative empty set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisteredProvider {
    pub plugin_id: String,
    pub operations: BTreeSet<FsOperation>,
    pub reported: bool,
    pub discovery: bool,
    pub preflight: bool,
    instances: BTreeMap<String, vfs_instances::VfsInstance>,
}

impl RegisteredProvider {
    pub fn supports(&self, operation: FsOperation) -> bool {
        self.operations.contains(&operation)
    }

    pub fn declares(&self, operation: FsOperation) -> bool {
        self.reported && self.operations.contains(&operation)
    }
}

fn registry() -> &'static Mutex<HashMap<String, RegisteredProvider>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, RegisteredProvider>>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

/// Record the `vfs` block of a `plugin.initialize` reply. Unknown operation
/// names are ignored so a newer plugin cannot break an older Core.
pub fn register(plugin_id: &str, reply: &Value) -> RegisteredProvider {
    let mut provider = RegisteredProvider {
        plugin_id: plugin_id.to_string(),
        ..RegisteredProvider::default()
    };
    if let Some(operations) = reply
        .get("vfs")
        .and_then(|vfs| vfs.get("operations"))
        .and_then(Value::as_array)
    {
        provider.reported = true;
        for value in operations.iter().filter_map(Value::as_str) {
            if let Some(operation) = FsOperation::parse(value) {
                provider.operations.insert(operation);
            }
        }
    }
    provider.preflight = reply
        .pointer("/vfs/preflight")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    provider.discovery = reply
        .pointer("/vfs/discovery")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match vfs_instances::read(reply, &provider.operations, provider.preflight) {
        Ok(instances) => provider.instances = instances,
        Err(error) => {
            // A malformed handshake never grants the union capability set.
            eprintln!("VFS capability handshake rejected for {plugin_id}: {error}");
            provider.reported = false;
            provider.operations.clear();
            provider.preflight = false;
            provider.discovery = false;
        }
    }
    if let Ok(mut registry) = registry().lock() {
        registry.insert(plugin_id.to_string(), provider.clone());
    }
    provider
}

pub fn forget(plugin_id: &str) {
    if let Ok(mut registry) = registry().lock() {
        registry.remove(plugin_id);
    }
}

pub fn get(plugin_id: &str) -> Option<RegisteredProvider> {
    registry()
        .lock()
        .ok()
        .and_then(|registry| registry.get(plugin_id).cloned())
}

pub fn supports(plugin_id: &str, operation: FsOperation) -> bool {
    get(plugin_id).is_some_and(|provider| provider.declares(operation))
}

/// Bind a named VFS implementation to the actual registered Core mount,
/// not to the providerId supplied by an arbitrary JSON request.
pub fn bind_fs_instance(plugin_id: &str, params: &mut Value) -> Result<(), String> {
    vfs_instances::bind(plugin_id, params)
}

pub fn supports_mount(plugin_id: &str, mount_id: &str, operation: FsOperation) -> bool {
    vfs_instances::supports(plugin_id, mount_id, operation)
}

/// A verified mount's declared operations, for one consistent UI capability
/// snapshot. Unlike operations(plugin_id), this never returns a package union.
pub fn operations_mount(plugin_id: &str, mount_id: &str) -> Option<Vec<FsOperation>> {
    vfs_instances::operations(plugin_id, mount_id)
}

pub fn preflight_mount(plugin_id: &str, mount_id: &str) -> bool {
    vfs_instances::preflight(plugin_id, mount_id)
}

/// Gate one outgoing method against the declaration. Non-file methods pass.
pub fn authorize_method(plugin_id: &str, method: &str) -> Result<(), String> {
    let Some(operation) = FsOperation::from_method(method) else {
        return Ok(());
    };
    match get(plugin_id) {
        None => Err(format!(
            "plugin {plugin_id} has not declared file-system capabilities"
        )),
        Some(provider) if !provider.reported => Err(format!(
            "plugin {plugin_id} did not report file-system capabilities"
        )),
        Some(provider) if !provider.supports(operation) => Err(format!(
            "plugin {plugin_id} does not implement fs operation {}",
            operation.as_str()
        )),
        Some(_) => Ok(()),
    }
}

/// Discovery is negotiated and only precedes new logical operations. Existing
/// streams must always be able to release their handles with close/abort.
pub fn needs_discovery(plugin_id: &str, method: &str) -> bool {
    !matches!(
        method,
        "vfs.readStream" | "vfs.closeRead" | "vfs.writeStream" | "vfs.commit" | "vfs.abort"
    ) && FsOperation::from_method(method).is_some()
        && get(plugin_id).is_some_and(|provider| provider.discovery)
}

pub fn authorize_discovered(method: &str, reply: &Value) -> Result<(), String> {
    let operation = FsOperation::from_method(method).ok_or("invalid discovered operation")?;
    let allowed = reply
        .get("operations")
        .and_then(Value::as_array)
        .ok_or("provider discovery is missing operations")?;
    if allowed
        .iter()
        .any(|value| value.as_str() == Some(operation.as_str()))
    {
        Ok(())
    } else {
        Err(format!(
            "server does not permit {} for this resource",
            operation.as_str()
        ))
    }
}

/// Operations a plugin implements, for UI decisions such as which builtin
/// menu entries may appear. `None` means the plugin never declared anything.
pub fn operations(plugin_id: &str) -> Option<Vec<FsOperation>> {
    get(plugin_id)
        .filter(|provider| provider.reported)
        .map(|provider| provider.operations.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn declaration_follows_the_handshake() {
        let provider = register(
            "org.example.drive",
            &json!({"pluginId":"org.example.drive","ready":true,"vfs":{"operations":["list","stat","read"]}}),
        );
        assert!(provider.reported);
        assert_eq!(provider.operations.len(), 3);
        assert!(supports("org.example.drive", FsOperation::List));
        assert!(!supports("org.example.drive", FsOperation::Rename));
        forget("org.example.drive");
        assert!(!supports("org.example.drive", FsOperation::List));
    }

    #[test]
    fn unknown_operations_are_ignored() {
        let provider = register(
            "org.example.future",
            &json!({"vfs":{"operations":["list","teleport"]}}),
        );
        assert_eq!(
            provider.operations.into_iter().collect::<Vec<_>>(),
            vec![FsOperation::List]
        );
        forget("org.example.future");
    }

    #[test]
    fn undeclared_plugins_cannot_receive_file_requests() {
        forget("org.example.legacy");
        assert!(authorize_method("org.example.legacy", "vfs.list").is_err());
        register(
            "org.example.legacy",
            &json!({"pluginId":"org.example.legacy"}),
        );
        assert!(authorize_method("org.example.legacy", "vfs.list").is_err());
        assert!(authorize_method("org.example.legacy", "connection.open").is_ok());
        forget("org.example.legacy");
    }

    #[test]
    fn discovery_gates_new_operations_without_orphaning_streams() {
        let plugin = "org.example.discovery";
        register(
            plugin,
            &json!({"vfs":{"operations":["read","write"],"discovery":true}}),
        );
        assert!(needs_discovery(plugin, "vfs.openRead"));
        assert!(needs_discovery(plugin, "vfs.beginWrite"));
        for method in [
            "vfs.discover",
            "vfs.closeRead",
            "vfs.abort",
            "vfs.commit",
            "vfs.readStream",
        ] {
            assert!(!needs_discovery(plugin, method));
        }
        assert!(authorize_discovered("vfs.beginWrite", &json!({"operations":["read"]})).is_err());
        assert!(authorize_discovered("vfs.openRead", &json!({"operations":[]})).is_err());
        assert!(authorize_discovered("vfs.openRead", &json!({"operations":["read"]})).is_ok());
        assert!(authorize_discovered("vfs.openRead", &Value::Null).is_err());
        forget(plugin);
    }

    #[test]
    fn streaming_methods_map_to_their_capability() {
        assert_eq!(
            FsOperation::from_method("vfs.readStream"),
            Some(FsOperation::Read)
        );
        assert_eq!(
            FsOperation::from_method("vfs.commit"),
            Some(FsOperation::Write)
        );
        assert_eq!(FsOperation::from_method("fs.stat"), None);
    }
}
