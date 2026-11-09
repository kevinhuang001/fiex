//! Persistent KV/CAS storage shared by Core Modules and authenticated plugins.
//! Provider IDs are deliberately not storage owners: all providers registered
//! by one module use that module's OwnerId.
mod engine;
pub mod native;
mod owner;

pub use owner::CoreOwnerKey;

pub fn plugin_validate(plugin_id: &str, scope: &str, key: &str) -> Result<(), String> {
    engine::validate(&owner::OwnerId::plugin(plugin_id), scope, key)
}

pub fn plugin_get(
    plugin_id: &str,
    scope: &str,
    key: &str,
) -> Result<Option<serde_json::Value>, String> {
    engine::get(&owner::OwnerId::plugin(plugin_id), scope, key)
}

pub fn plugin_set(
    plugin_id: &str,
    scope: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    engine::set(&owner::OwnerId::plugin(plugin_id), scope, key, value)
}

pub fn plugin_delete(plugin_id: &str, scope: &str, key: &str) -> Result<(), String> {
    engine::delete(&owner::OwnerId::plugin(plugin_id), scope, key)
}

pub fn plugin_keys(plugin_id: &str, scope: &str) -> Result<Vec<String>, String> {
    engine::list_keys(&owner::OwnerId::plugin(plugin_id), scope)
}

pub fn plugin_compare_and_set(
    plugin_id: &str,
    scope: &str,
    key: &str,
    expected: Option<serde_json::Value>,
    next: Option<serde_json::Value>,
) -> Result<bool, String> {
    engine::compare_and_set(
        &owner::OwnerId::plugin(plugin_id),
        scope,
        key,
        expected,
        next,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        engine,
        native::OwnerBoundStorage,
        owner::{CoreOwnerKey, OwnerId},
    };
    use serde_json::json;

    #[test]
    fn core_modules_and_plugins_have_separate_storage_owners() {
        let applications = OwnerBoundStorage::from_registry(CoreOwnerKey::new("owner-a"))
            .scoped("owner-test")
            .unwrap();
        let recent = OwnerBoundStorage::from_registry(CoreOwnerKey::new("owner-b"))
            .scoped("owner-test")
            .unwrap();
        let plugin = OwnerId::plugin("org.example.module");
        let value = json!({"owner":"applications"});
        assert!(
            applications
                .compare_and_set("key", None, Some(value.clone()))
                .unwrap()
        );
        assert_eq!(applications.get("key").unwrap(), Some(value));
        assert_eq!(recent.get("key").unwrap(), None);
        assert_eq!(engine::get(&plugin, "owner-test", "key").unwrap(), None);
        applications.delete("key").unwrap();
    }
}
