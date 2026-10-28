//! ProviderRegistry: the capabilities a plugin declared during
//! `plugin.initialize`.
//!
//! Core never guesses what a provider can do. A plugin reports the operations
//! it really implements, Core stores that set per plugin, and every `vfs.*`
//! request is checked against it before it leaves the process boundary. A
//! plugin that supports `rename` for one account can still restrict a single
//! node through `allowedOps`; user grants and the Operation Scope are checked
//! independently by the permission layer.
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Mutex, OnceLock},
};

/// Every file-system operation in protocol 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FsOperation {
    List,
    Stat,
    Read,
    Write,
    Mkdir,
    Rename,
    Remove,
    Search,
}

impl FsOperation {
    pub const ALL: [FsOperation; 8] = [
        FsOperation::List,
        FsOperation::Stat,
        FsOperation::Read,
        FsOperation::Write,
        FsOperation::Mkdir,
        FsOperation::Rename,
        FsOperation::Remove,
        FsOperation::Search,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FsOperation::List => "list",
            FsOperation::Stat => "stat",
            FsOperation::Read => "read",
            FsOperation::Write => "write",
            FsOperation::Mkdir => "mkdir",
            FsOperation::Rename => "rename",
            FsOperation::Remove => "remove",
            FsOperation::Search => "search",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        FsOperation::ALL
            .into_iter()
            .find(|operation| operation.as_str() == value)
    }

    /// Capability behind a wire method. Streaming sub-methods belong to the
    /// operation that opened the stream.
    pub fn from_method(method: &str) -> Option<Self> {
        FsOperation::ALL.into_iter().find(|operation| {
            let methods: &[&str] = match operation {
                FsOperation::List => &["vfs.list"],
                FsOperation::Stat => &["vfs.stat"],
                FsOperation::Read => &["vfs.openRead", "vfs.readStream", "vfs.closeRead"],
                FsOperation::Write => &[
                    "vfs.beginWrite",
                    "vfs.writeStream",
                    "vfs.commit",
                    "vfs.abort",
                ],
                FsOperation::Mkdir => &["vfs.mkdir"],
                FsOperation::Rename => &["vfs.rename"],
                FsOperation::Remove => &["vfs.remove"],
                FsOperation::Search => &["vfs.search"],
            };
            methods.contains(&method)
        })
    }
}

/// One plugin's declaration. `reported` distinguishes "declared nothing" from
/// "never handshook": only the former is an authoritative empty set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisteredProvider {
    pub plugin_id: String,
    pub operations: BTreeSet<FsOperation>,
    pub reported: bool,
    pub discovery: bool,
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
    provider.discovery = reply
        .pointer("/vfs/discovery")
        .and_then(Value::as_bool)
        .unwrap_or(false);
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
        assert_eq!(FsOperation::from_method("host.fs.stat"), None);
    }
}
