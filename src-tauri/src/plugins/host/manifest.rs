//! Plugin manifest schema, install-time validation and permission declarations.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path};

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

pub fn declared_permissions(manifest: &PluginManifest) -> Result<BTreeSet<String>, String> {
    validate_extensions(&manifest.extensions)?;
    let mut declared = BTreeSet::new();
    let folders = &manifest.permissions.folders;
    let files = &manifest.permissions.files;
    if manifest.permissions.metadata.enabled {
        declared.insert("metadata".to_string());
    }
    if folders.read.enabled {
        declared.insert("folders.read".to_string());
    }
    for (name, requested) in [
        ("folders.create", folders.create.enabled),
        ("folders.delete", folders.delete.enabled),
        ("folders.rename", folders.rename.enabled),
    ] {
        if requested {
            declared.insert(name.into());
        }
    }
    if files.read.enabled {
        declared.insert("files.read".to_string());
    }
    if files.write.enabled {
        declared.insert("files.write".to_string());
    }
    for (name, requested) in [
        ("files.create", files.create.enabled),
        ("files.delete", files.delete.enabled),
        ("files.rename", files.rename.enabled),
    ] {
        if requested {
            declared.insert(name.into());
        }
    }
    if manifest.permissions.network.enabled
        || manifest.permissions.network.all
        || !manifest.permissions.network.domains.is_empty()
    {
        declared.insert("network".to_string());
    }
    if manifest.permissions.commands.enabled {
        declared.insert("commands".to_string());
    }
    if manifest.permissions.hooks.enabled {
        declared.insert("hooks".to_string());
    }
    crate::plugins::security::permissions::validate_commands(
        &declared,
        &manifest.permissions.commands.allow,
    )?;
    crate::plugins::security::permissions::validate_network_domains(
        &declared,
        &manifest.permissions.network.domains,
    )?;
    Ok(declared)
}

pub(crate) fn declared(manifest: &PluginManifest) -> Result<BTreeSet<String>, String> {
    declared_permissions(manifest)
}

pub fn activation_requirements(
    manifest: &PluginManifest,
) -> Result<crate::plugins::security::permissions::ActivationConsent, String> {
    let permissions = declared(manifest)?;
    let domains = crate::plugins::security::permissions::validate_network_domains(
        &permissions,
        &manifest.permissions.network.domains,
    )?;
    let commands = crate::plugins::security::permissions::validate_commands(
        &permissions,
        &manifest.permissions.commands.allow,
    )?;
    Ok(crate::plugins::security::permissions::ActivationConsent {
        permissions,
        network_all: manifest.permissions.network.all,
        domains,
        commands,
    })
}

pub fn valid_id(id: &str) -> bool {
    id.len() > 2
        && id.len() < 128
        && id.contains('.')
        && !id.starts_with('.')
        && !id.contains("..")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// A declared package entry must be a relative slash-separated path inside the
/// plugin package. URLs, absolute paths, drive letters, parent segments and
/// backslashes are never resolved by Core; reads also re-check canonical
/// containment so a symlink cannot escape the package.
pub(crate) fn valid_package_entry(entry: &str) -> bool {
    if entry.is_empty()
        || entry.len() > 1024
        || entry.contains("://")
        || entry.contains(':')
        || entry.contains('\\')
        || entry.contains('\0')
    {
        return false;
    }
    let path = Path::new(entry);
    path.is_relative()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Optional `extensions.preview.loading.entry`, already validated by
/// [`validate_extensions`] when the manifest is discovered.
pub(crate) fn preview_loading_entry(extensions: &Value) -> Option<&str> {
    extensions
        .get("preview")
        .and_then(Value::as_object)
        .and_then(|preview| preview.get("loading"))
        .and_then(Value::as_object)
        .and_then(|loading| loading.get("entry"))
        .and_then(Value::as_str)
}

/// Extensions declare provider eligibility, never resource permissions.
pub fn validate_extensions(value: &Value) -> Result<(), String> {
    if !value.is_null() && !value.is_object() {
        return Err("extensions must be an object".into());
    }
    if let Some(enabled) = value.get("contextMenus") {
        match enabled {
            Value::Bool(_) => {}
            Value::Object(object) => {
                crate::plugins::match_rules::validate(object.get("match"), false, true)?
            }
            _ => return Err("extensions.contextMenus must be true or {match}".into()),
        }
    }
    if let Some(decorations) = value.get("decorations") {
        match decorations {
            Value::Bool(_) => {}
            Value::Object(object) => {
                for forbidden in ["mode", "method", "scope", "rules"] {
                    if object.contains_key(forbidden) {
                        return Err(format!(
                            "extensions.decorations.{forbidden} is removed; decorations are dynamic and use decorations.resolve"
                        ));
                    }
                }
                crate::plugins::match_rules::validate(object.get("match"), true, false)?;
            }
            _ => return Err("extensions.decorations must be true or {match,priority}".into()),
        }
    }
    for key in ["info", "preview"] {
        let Some(item) = value.get(key) else {
            continue;
        };
        let object = item
            .as_object()
            .ok_or_else(|| format!("extensions.{key} must be an object"))?;
        if object.contains_key("title") {
            return Err(format!(
                "extensions.{key}.title is removed; the plugin name is used"
            ));
        }
        if key == "info" && object.contains_key("id") {
            return Err("extensions.info.id is not used; remove it".into());
        }
        if key == "info" && object.contains_key("command") {
            return Err("extensions.info.command is removed; info.resolve is fixed".into());
        }
        crate::plugins::match_rules::validate(object.get("match"), false, false)?;
        if key == "preview" {
            if let Some(loading) = object.get("loading") {
                let loading = loading
                    .as_object()
                    .ok_or("extensions.preview.loading must be an object")?;
                for field in loading.keys() {
                    if field != "entry" {
                        return Err(format!(
                            "extensions.preview.loading.{field} is not supported"
                        ));
                    }
                }
                let entry = loading
                    .get("entry")
                    .and_then(Value::as_str)
                    .ok_or("extensions.preview.loading.entry is required")?;
                if !valid_package_entry(entry) {
                    return Err(
                        "extensions.preview.loading.entry must be a package-relative path".into(),
                    );
                }
            }
        }
    }
    for key in ["search", "hooks"] {
        let Some(item) = value.get(key) else {
            continue;
        };
        let object = item
            .as_object()
            .ok_or_else(|| format!("extensions.{key} must be an object"))?;
        if object.contains_key("id") {
            return Err(format!(
                "extensions.{key}.id is not used; the plugin id is the contribution id"
            ));
        }
        if key == "search" && (object.contains_key("title") || object.contains_key("command")) {
            return Err(
                "extensions.search.title/command are removed; search.query is fixed".into(),
            );
        }
        crate::plugins::match_rules::validate(object.get("match"), false, false)?;
        if key == "hooks" && object.contains_key("method") {
            return Err("extensions.hooks.method is removed; hooks.invoke is fixed".into());
        }
    }
    if let Some(themes) = value.get("themes") {
        let object = themes
            .as_object()
            .ok_or("extensions.themes must be an object")?;
        if object.contains_key("id") || object.contains_key("title") {
            return Err(
                "extensions.themes.id/title are not used; the theme file carries them".into(),
            );
        }
        if object
            .get("path")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err("extensions.themes.path is required".into());
        }
    }
    if let Some(ui) = value.get("ui") {
        let object = ui.as_object().ok_or("extensions.ui must be an object")?;
        for key in object.keys() {
            if key != "timeoutMs" {
                return Err(format!("extensions.ui.{key} is not supported"));
            }
        }
        if let Some(timeout) = object.get("timeoutMs") {
            let timeout = timeout
                .as_u64()
                .ok_or("extensions.ui.timeoutMs must be a positive integer")?;
            if !(MIN_UI_TIMEOUT_MS..=MAX_UI_TIMEOUT_MS).contains(&timeout) {
                return Err(format!(
                    "extensions.ui.timeoutMs must be between {MIN_UI_TIMEOUT_MS} and {MAX_UI_TIMEOUT_MS}"
                ));
            }
        }
    }
    for key in ["vfs", "connections", "toolbar"] {
        let Some(item) = value.get(key) else {
            continue;
        };
        let object = item
            .as_object()
            .ok_or_else(|| format!("extensions.{key} must be an object"))?;
        if object.contains_key("id") {
            return Err(format!(
                "extensions.{key}.id is not used; the plugin id is the contribution id"
            ));
        }
        for forbidden in [
            "scheme",
            "schemes",
            "uriPrefix",
            "mountId",
            "mountPath",
            "capabilities",
        ] {
            if object.contains_key(forbidden) {
                return Err(format!(
                    "extensions.{key}.{forbidden} is forbidden; Core owns VFS addresses"
                ));
            }
        }
        if object
            .get("title")
            .and_then(Value::as_str)
            .is_none_or(|s| s.trim().is_empty() || s.len() > 256)
        {
            return Err(format!("extensions.{key}.title is required"));
        }
        if key == "toolbar" && object.contains_key("command") {
            return Err("extensions.toolbar.command is removed; toolbar.invoke is fixed".into());
        }
        if key == "connections" && !object.get("fields").is_some_and(Value::is_array) {
            return Err("extensions.connections.fields must be an array".into());
        }
        if object.get("readOnly").is_some_and(|v| !v.is_boolean()) {
            return Err(format!("extensions.{key}.readOnly must be a boolean"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod contribution_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn removed_top_level_capabilities_are_not_silently_accepted() {
        let mut manifest =
            json!({"schemaVersion":2,"id":"test.plugin","name":"Test","version":"1"});
        assert!(serde_json::from_value::<PluginManifest>(manifest.clone()).is_ok());
        manifest["capabilities"] = json!(["connection"]);
        let error = serde_json::from_value::<PluginManifest>(manifest)
            .unwrap_err()
            .to_string();
        assert!(error.contains("capabilities"));
    }
    #[test]
    fn provider_declarations_are_explicit_and_unambiguous() {
        for value in [
            json!({}),
            json!({"vfs": {"title": "History"}}),
            json!({"connections": {"title": "Drive", "fields": []}}),
            json!({"toolbar": {"title": "Git"}}),
            json!({"search": {"icon": "magnifying-glass"}}),
            json!({"hooks": {"event": "beforeOpen"}}),
        ] {
            assert!(validate_extensions(&value).is_ok(), "{value}");
        }
        for value in [
            json!({"vfs": []}),
            json!({"vfs": {}}),
            json!({"vfs": {"id": "history", "title": "History"}}),
            json!({"vfs": [{"title": "History"}]}),
            json!({"connections": {"id": "drive", "title": "Drive", "fields": []}}),
            json!({"connections": [{"title": "Drive", "fields": []}]}),
            json!({"toolbar": {"id": "git.main", "title": "Git"}}),
            json!({"toolbar": {"schemes": ["history"], "title": "Git"}}),
            json!({"search": []}),
            json!({"search": {"id": "search"}}),
            json!({"hooks": []}),
            json!({"hooks": {"id": "beforeOpen", "event": "beforeOpen"}}),
        ] {
            assert!(validate_extensions(&value).is_err(), "{value}");
        }
    }
    #[test]
    fn ui_timeout_is_an_explicit_bounded_contract() {
        assert_eq!(ui_timeout_ms(&json!({})), DEFAULT_UI_TIMEOUT_MS);
        assert_eq!(
            ui_timeout_ms(&json!({"ui": {"timeoutMs": 300_000}})),
            300_000
        );
        // The process request budget keeps its historical default and can only
        // be extended by the manifest contract.
        assert_eq!(request_timeout_ms(&json!({})), DEFAULT_REQUEST_TIMEOUT_MS);
        assert_eq!(
            request_timeout_ms(&json!({"ui": {"timeoutMs": 1_000}})),
            DEFAULT_REQUEST_TIMEOUT_MS
        );
        assert_eq!(
            request_timeout_ms(&json!({"ui": {"timeoutMs": 300_000}})),
            300_000
        );
        for invalid in [
            json!({"ui": []}),
            json!({"ui": {"timeoutMs": "long"}}),
            json!({"ui": {"timeoutMs": 0}}),
            json!({"ui": {"timeoutMs": MAX_UI_TIMEOUT_MS + 1}}),
            json!({"ui": {"timeoutMs": 30_000, "extra": true}}),
        ] {
            assert!(validate_extensions(&invalid).is_err(), "{invalid}");
        }
        assert!(validate_extensions(&json!({"ui": {"timeoutMs": 300_000}})).is_ok());
    }

    #[test]
    fn menu_registration_rejects_static_items_and_nested_syntax() {
        assert!(validate_extensions(&serde_json::json!({"contextMenus": true})).is_ok());
        assert!(validate_extensions(&serde_json::json!({"contextMenus": false})).is_ok());
        for invalid in [
            serde_json::json!({"contextMenus": []}),
            serde_json::json!({"contextMenus": [{"when":"context.selection.count > 0"}]}),
            serde_json::json!({"contextMenus": [{"children":[]}]}),
        ] {
            assert!(validate_extensions(&invalid).is_err());
        }
    }
    #[test]
    fn match_predicates_are_scoped_by_extension_point() {
        assert!(validate_extensions(&serde_json::json!({
            "decorations": {"match": {"contains": [".git"]}}
        }))
        .is_ok());
        assert!(validate_extensions(&serde_json::json!({
            "contextMenus": {"match": {"selection": "single"}}
        }))
        .is_ok());
        assert!(validate_extensions(&serde_json::json!({
            "info": {"match": {"mimeTypes": ["text/*"]}}
        }))
        .is_ok());
        for invalid in [
            serde_json::json!({"decorations": {"match": {"selection": "single"}}}),
            serde_json::json!({"contextMenus": {"match": {"contains": [".git"]}}}),
            serde_json::json!({"info": {"match": {"contains": [".git"]}}}),
            serde_json::json!({"info": [{"id": "x"}]}),
            serde_json::json!({"info": {"title": "X"}}),
            serde_json::json!({"info": {"id": "x"}}),
            serde_json::json!({"contextMenus": {"match": {"selection": "many"}}}),
        ] {
            assert!(validate_extensions(&invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn preview_loading_pages_are_optional_and_package_local() {
        for valid in [
            json!({"preview": {}}),
            json!({"preview": {"panel": {"entry": "ui/preview.html"}}}),
            json!({"preview": {"loading": {"entry": "ui/loading.html"}}}),
            json!({"preview": {
                "panel": {"entry": "ui/preview.html"},
                "loading": {"entry": "ui/loading.html"}
            }}),
        ] {
            assert!(validate_extensions(&valid).is_ok(), "{valid}");
        }
        for invalid in [
            json!({"preview": {"loading": {}}}),
            json!({"preview": {"loading": true}}),
            json!({"preview": {"loading": []}}),
            json!({"preview": {"loading": {"entry": ""}}}),
            json!({"preview": {"loading": {"entry": "../loading.html"}}}),
            json!({"preview": {"loading": {"entry": "./loading.html"}}}),
            json!({"preview": {"loading": {"entry": "/tmp/loading.html"}}}),
            json!({"preview": {"loading": {"entry": "C:/loading.html"}}}),
            json!({"preview": {"loading": {"entry": "https://example.com/loading.html"}}}),
            json!({"preview": {"loading": {"entry": "ui\\loading.html"}}}),
            json!({"preview": {"loading": {"entry": "ui/loading.html", "url": "https://example.com"}}}),
        ] {
            assert!(validate_extensions(&invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn hooks_and_search_validate_match_instead_of_ignoring_bad_rules() {
        for feature in ["hooks", "search"] {
            assert!(
                validate_extensions(&json!({feature:{"match":{"extensions":["zip"]}}})).is_ok()
            );
            for predicate in [
                json!({"extensions":"zip"}),
                json!({"selection":"single"}),
                json!({"contains":[".git"]}),
                json!({"extension":["zip"]}),
            ] {
                assert!(validate_extensions(&json!({feature:{"match":predicate}})).is_err());
            }
        }
    }

    #[test]
    fn metadata_and_file_operations_are_independent_grants() {
        let manifest: PluginManifest = serde_json::from_value(json!({
            "schemaVersion": 2,
            "id": "test.permissions", "name": "Permissions", "version": "1",
            "extensions": {},
            "permissions": {
                "metadata": {"enabled": true},
                "files": {
                    "read": {"enabled": false},
                    "create": {"enabled": true},
                    "delete": {"enabled": true}
                }
            }
        }))
        .unwrap();
        let grants = declared_permissions(&manifest).unwrap();
        assert!(grants.contains("metadata"));
        assert!(grants.contains("files.create"));
        assert!(grants.contains("files.delete"));
        assert!(!grants.contains("files.read"));
        assert!(!grants.contains("folders.read"));
        assert!(serde_json::from_value::<PluginManifest>(json!({
            "schemaVersion": 2,
            "id": "test.legacy", "name": "Legacy", "version": "1",
            "contributions": {}
        }))
        .is_err());
    }
}
