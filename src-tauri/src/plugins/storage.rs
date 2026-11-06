//! Authenticated Plugin Module adapter for the shared ProviderStorage Engine.
//! The plugin owner comes from the verified host session; callers cannot
//! supply a Core OwnerId or another storage namespace.
use serde_json::Value;

pub(super) fn validate(plugin_id: &str, scope: &str, key: &str) -> Result<(), String> {
    fiex_storage::plugin_validate(plugin_id, scope, key)
}

pub fn compare_and_set(
    plugin_id: &str,
    scope: &str,
    key: &str,
    expected: Option<Value>,
    next: Option<Value>,
) -> Result<bool, String> {
    fiex_storage::plugin_compare_and_set(plugin_id, scope, key, expected, next)
}

pub fn get(plugin_id: &str, scope: &str, key: &str) -> Result<Option<Value>, String> {
    fiex_storage::plugin_get(plugin_id, scope, key)
}

pub fn set(plugin_id: &str, scope: &str, key: &str, value: Value) -> Result<(), String> {
    fiex_storage::plugin_set(plugin_id, scope, key, value)
}

pub fn delete(plugin_id: &str, scope: &str, key: &str) -> Result<(), String> {
    fiex_storage::plugin_delete(plugin_id, scope, key)
}

pub fn list_keys(plugin_id: &str, scope: &str) -> Result<Vec<String>, String> {
    fiex_storage::plugin_keys(plugin_id, scope)
}

pub use super::credentials::{
    credential_backend, secret_available, secret_delete, secret_get, secret_response, secret_set,
    set_credential_backend,
};
