//! Plugin manifest schema, install-time validation and permission declarations.
mod connections;
#[cfg(test)]
mod contribution_tests;
mod permissions;
mod validation;
mod provider_instances;
pub(crate) use permissions::declared;
pub use permissions::{activation_requirements, declared_permissions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
pub(crate) use validation::{preview_loading_entry, valid_package_entry};
pub use validation::{valid_id, validate_extensions};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntime {
    #[serde(default)]
    pub protocol_version: String,
    #[serde(default)]
    pub entrypoints: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderReadPermission {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderPermission {
    #[serde(default)]
    pub read: FolderReadPermission,
    #[serde(default)]
    pub create: OperationPermission,
    #[serde(default)]
    pub delete: OperationPermission,
    #[serde(default)]
    pub rename: OperationPermission,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationPermission {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePermission {
    #[serde(default)]
    pub read: OperationPermission,
    #[serde(default)]
    pub write: OperationPermission,
    #[serde(default)]
    pub create: OperationPermission,
    #[serde(default)]
    pub delete: OperationPermission,
    #[serde(default)]
    pub rename: OperationPermission,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandsPermission {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allow: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPermission {
    #[serde(default)]
    pub enabled: bool,
    /// Explicit "all hosts" access. The UI marks this as dangerous and can
    /// switch the plugin to a whitelist instead.
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPermissions {
    /// Basic file and folder attributes; neither content nor enumeration.
    #[serde(default)]
    pub metadata: MetadataPermission,
    #[serde(default)]
    pub folders: FolderPermission,
    #[serde(default)]
    pub files: FilePermission,
    #[serde(default)]
    pub network: NetworkPermission,
    #[serde(default)]
    pub commands: CommandsPermission,
    /// Allows `extensions.hooks` to run before Core acts on a file.
    #[serde(default)]
    pub hooks: HooksPermission,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataPermission {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksPermission {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPublisher {
    pub id: String,
    pub name: String,
    pub key_id: String,
    pub public_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Source-message catalogs for declarative display fields only.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub locales: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub runtime: PluginRuntime,
    #[serde(default)]
    pub permissions: PluginPermissions,
    #[serde(default)]
    pub extensions: Value,
    #[serde(default)]
    pub author: Option<PluginAuthor>,
    #[serde(default)]
    pub publisher: Option<PluginPublisher>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAuthor {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// Default budget for one plugin UI request and its bridge handshake.
pub const DEFAULT_UI_TIMEOUT_MS: u64 = 15_000;
/// Plugin process requests keep the historical budget unless a manifest asks
/// for more. `extensions.ui.timeoutMs` can only extend this, never shrink it.
pub const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 120_000;
/// A plugin can ask for a longer budget for slow providers (for example an
/// archive directory listing), but never an unbounded one.
pub const MAX_UI_TIMEOUT_MS: u64 = 900_000;
const MIN_UI_TIMEOUT_MS: u64 = 1_000;

fn declared_ui_timeout(extensions: &Value) -> Option<u64> {
    extensions
        .get("ui")
        .and_then(|ui| ui.get("timeoutMs"))
        .and_then(Value::as_u64)
        .filter(|value| (MIN_UI_TIMEOUT_MS..=MAX_UI_TIMEOUT_MS).contains(value))
}

/// Explicit `extensions.ui.timeoutMs` contract. Core applies it to the panel
/// bridge handshake and projects it into the UI context so the plugin's own
/// request timer can match it.
pub fn ui_timeout_ms(extensions: &Value) -> u64 {
    declared_ui_timeout(extensions).unwrap_or(DEFAULT_UI_TIMEOUT_MS)
}

/// Request deadline for one plugin RPC. A manifest may raise the default for
/// slow providers, but an absent or tiny value keeps the standard budget.
pub fn request_timeout_ms(extensions: &Value) -> u64 {
    declared_ui_timeout(extensions).map_or(DEFAULT_REQUEST_TIMEOUT_MS, |value| {
        value.max(DEFAULT_REQUEST_TIMEOUT_MS)
    })
}
