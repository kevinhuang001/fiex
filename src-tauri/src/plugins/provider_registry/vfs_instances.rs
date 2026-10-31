//! Core-owned selection of a named VFS implementation after the authenticated
//! plugin.initialize handshake. Neither UI payloads nor plugin replies may
//! select another account's mount or borrow an unrelated provider's grants.
use super::FsOperation;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VfsInstance {
    pub operations: BTreeSet<FsOperation>,
    pub preflight: bool,
}

pub(super) fn read(
    reply: &Value,
    declared: &BTreeSet<FsOperation>,
    preflight: bool,
) -> Result<BTreeMap<String, VfsInstance>, String> {
    let Some(value) = reply.pointer("/vfs/providers") else {
        return Ok(BTreeMap::new());
    };
    let items = value.as_array().ok_or("vfs.providers must be an array")?;
    if items.is_empty() || items.len() > 32 {
        return Err("vfs.providers requires 1..32 instances".into());
    }
    let mut instances = BTreeMap::new();
    for item in items {
        let id = item["id"].as_str().ok_or("vfs provider ID missing")?;
        if id.is_empty()
            || id.len() > 128
            || id.starts_with("core:")
            || !id.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-:".contains(&c))
            || instances.contains_key(id)
        {
            return Err("invalid or duplicate VFS provider ID".into());
        }
        let operations = item["operations"]
            .as_array()
            .ok_or("vfs instance operations missing")?;
        let mut selected = BTreeSet::new();
        for value in operations {
            let operation = value.as_str().and_then(FsOperation::parse)
                .ok_or("unknown VFS provider operation")?;
            if !declared.contains(&operation) {
                return Err("VFS instance exceeds negotiated operation ceiling".into());
            }
            selected.insert(operation);
        }
        let enabled = item["preflight"].as_bool().ok_or("vfs preflight must be boolean")?;
        if enabled && !preflight {
            return Err("VFS instance preflight exceeds negotiated ceiling".into());
        }
        instances.insert(id.into(), VfsInstance { operations: selected, preflight: enabled });
    }
    Ok(instances)
}

fn mount_owner(plugin_id: &str, mount_id: &str) -> Result<String, String> {
    let path = crate::plugins::mounts::uri(mount_id, "")?;
    let (_, source) = crate::plugins::mounts::resolve(&path)?;
    match source {
        crate::plugins::mounts::Source::Connection(connection)
            if connection.plugin_id == plugin_id => Ok(connection.kind),
        crate::plugins::mounts::Source::View(view)
            if view.descriptor.plugin_id == plugin_id => Ok(view.descriptor.contribution_id),
        _ => Err("VFS provider does not own the selected mount".into()),
    }
}

/// Select the instance only after Core has established the actual mount owner.
/// A single instance needs no providerId; SDK resolves it unambiguously.
/// For multiple instances, always erase the caller's ID and bind Core's own.
pub fn bind(plugin_id: &str, params: &mut Value) -> Result<(), String> {
    let state = super::get(plugin_id).ok_or("VFS provider handshake not complete")?;
    let fields = params.as_object_mut().ok_or("invalid VFS parameters")?;
    fields.remove("providerId");
    if state.instances.len() <= 1 {
        return Ok(());
    }
    let mount_id = fields.get("mountId").and_then(Value::as_str)
        .ok_or("VFS operation has no Core-bound mount")?;
    let selected = mount_owner(plugin_id, mount_id)?;
    if !state.instances.contains_key(&selected) {
        return Err("mount has no registered VFS provider instance".into());
    }
    fields.insert("providerId".into(), Value::String(selected));
    Ok(())
}

pub fn supports(plugin_id: &str, mount_id: &str, operation: FsOperation) -> bool {
    let Some(state) = super::get(plugin_id) else { return false };
    if state.instances.len() <= 1 {
        return state.declares(operation);
    }
    let Ok(selected) = mount_owner(plugin_id, mount_id) else { return false };
    state.instances.get(&selected)
        .is_some_and(|instance| instance.operations.contains(&operation))
}

pub fn preflight(plugin_id: &str, mount_id: &str) -> bool {
    let Some(state) = super::get(plugin_id) else { return false };
    if state.instances.len() <= 1 {
        return state.preflight;
    }
    let Ok(selected) = mount_owner(plugin_id, mount_id) else { return false };
    state.instances.get(&selected).is_some_and(|instance| instance.preflight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rejects_forged_or_overprivileged_handshake_instances() {
        let ceiling = BTreeSet::from([FsOperation::Read]);
        for value in [
            json!({"vfs":{"providers":[{"id":"core:local-fs","operations":["read"],"preflight":false}]}}),
            json!({"vfs":{"providers":[{"id":"drive","operations":["write"],"preflight":false}]}}),
            json!({"vfs":{"providers":[{"id":"drive","operations":["read"],"preflight":false},{"id":"drive","operations":["read"],"preflight":false}]}}),
            json!({"vfs":{"providers":[{"id":"drive","operations":["read"],"preflight":true}]}}),
        ] {
            assert!(read(&value, &ceiling, false).is_err());
        }
    }

    #[test]
    fn caller_cannot_choose_a_foreign_vfs_instance() {
        let plugin_id = "org.example.multi-vfs";
        super::super::register(plugin_id, &json!({"vfs":{
            "operations":["read","stat"],"preflight":false,
            "providers":[
                {"id":"drive","operations":["read"],"preflight":false},
                {"id":"git","operations":["stat"],"preflight":false}
            ]
        }}));
        let mut params = json!({"mountId":"unregistered","providerId":"drive"});
        assert!(bind(plugin_id, &mut params).is_err());
        assert!(params.get("providerId").is_none());
        super::super::forget(plugin_id);
    }
}
