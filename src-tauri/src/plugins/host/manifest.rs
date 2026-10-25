//! Plugin manifest schema, install-time validation and permission declarations.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    if manifest.permissions.network.enabled || !manifest.permissions.network.domains.is_empty() {
        declared.insert("network".to_string());
    }
    if manifest.permissions.commands.enabled {
        declared.insert("commands".to_string());
    }
    if manifest.permissions.hooks.enabled {
        declared.insert("hooks".to_string());
    }
    crate::plugin_permissions::validate_commands(&declared, &manifest.permissions.commands.allow)?;
    crate::plugin_permissions::validate_network_domains(
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
) -> Result<crate::plugin_permissions::ActivationConsent, String> {
    let permissions = declared(manifest)?;
    let domains = crate::plugin_permissions::validate_network_domains(
        &permissions,
        &manifest.permissions.network.domains,
    )?;
    let commands = crate::plugin_permissions::validate_commands(
        &permissions,
        &manifest.permissions.commands.allow,
    )?;
    Ok(crate::plugin_permissions::ActivationConsent {
        permissions,
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

/// Extensions declare provider eligibility, never resource permissions.
pub fn validate_extensions(value: &Value) -> Result<(), String> {
    if !value.is_null() && !value.is_object() {
        return Err("extensions must be an object".into());
    }
    for key in ["vfs", "connections", "toolbar"] {
        let Some(items) = value.get(key) else {
            continue;
        };
        let items = items
            .as_array()
            .ok_or_else(|| format!("extensions.{key} must be an array"))?;
        let mut ids = BTreeSet::new();
        for (index, item) in items.iter().enumerate() {
            let field = format!("extensions.{key}[{index}]");
            let object = item
                .as_object()
                .ok_or_else(|| format!("{field} must be an object"))?;
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
                        "{field}.{forbidden} is forbidden; Core owns VFS addresses"
                    ));
                }
            }
            if key == "toolbar" {
                continue;
            }
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{field}.id is required"))?;
            if id.is_empty()
                || id.len() > 80
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                || !ids.insert(id)
            {
                return Err(format!("{field}.id is invalid or duplicated"));
            }
            if item
                .get("title")
                .and_then(Value::as_str)
                .is_none_or(|s| s.trim().is_empty() || s.len() > 256)
            {
                return Err(format!("{field}.title is required"));
            }
            if key == "connections" && !item.get("fields").is_some_and(Value::is_array) {
                return Err(format!("{field}.fields must be an array"));
            }
            if item.get("readOnly").is_some_and(|v| !v.is_boolean()) {
                return Err(format!("{field}.readOnly must be a boolean"));
            }
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
            json!({"vfs": []}),
            json!({"vfs":[{"id":"history","title":"History"}]}),
            json!({"connections":[{"id":"drive","title":"Drive","fields":[]}]}),
        ] {
            assert!(validate_extensions(&value).is_ok());
        }
        for value in [
            json!({"vfs":{}}),
            json!({"vfs":[{"id":"../x","title":"Bad"}]}),
            json!({"vfs":[{"id":"a","title":"A"},{"id":"a","title":"B"}]}),
            json!({"toolbar":[{"schemes":["history"]}]}),
            json!({"connections":[{"id":"x","title":"X","fields":[],"capabilities":{"sync":true}}]}),
            json!({"connections":[{"id":"a","title":"A","fields":[],"scheme":"drive"}]}),
        ] {
            assert!(validate_extensions(&value).is_err());
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
