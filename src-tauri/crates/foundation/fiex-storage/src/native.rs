//! Native Adapter for trusted Core Modules. This is a direct function adapter
//! into the same storage Engine used by the Plugin IPC adapter.
use super::{
    engine,
    owner::{CoreOwnerKey, OwnerId},
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

#[derive(Clone)]
pub struct OwnerBoundStorage {
    owner: OwnerId,
}

impl OwnerBoundStorage {
    pub const fn from_registry(key: CoreOwnerKey) -> Self {
        Self {
            owner: OwnerId::core(key),
        }
    }

    pub fn scoped(&self, scope: &str) -> Result<NativeScopedStorage, String> {
        if scope.is_empty()
            || scope.len() > 128
            || !scope
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            || scope == "."
            || scope == ".."
            || scope.starts_with('.')
        {
            return Err("invalid storage scope".into());
        }
        Ok(NativeScopedStorage {
            owner: self.owner.clone(),
            scope: scope.to_owned(),
        })
    }
}

#[derive(Clone)]
pub struct NativeScopedStorage {
    owner: OwnerId,
    scope: String,
}

impl NativeScopedStorage {
    pub fn get(&self, key: &str) -> Result<Option<Value>, String> {
        engine::get(&self.owner, &self.scope, key)
    }

    pub fn set(&self, key: &str, value: Value) -> Result<(), String> {
        engine::set(&self.owner, &self.scope, key, value)
    }

    #[allow(dead_code)] // Parity with the plugin Storage adapter.
    pub fn delete(&self, key: &str) -> Result<(), String> {
        engine::delete(&self.owner, &self.scope, key)
    }

    #[allow(dead_code)] // Parity with the plugin Storage adapter.
    pub fn keys(&self) -> Result<Vec<String>, String> {
        engine::list_keys(&self.owner, &self.scope)
    }

    pub fn compare_and_set(
        &self,
        key: &str,
        expected: Option<Value>,
        next: Option<Value>,
    ) -> Result<bool, String> {
        engine::compare_and_set(&self.owner, &self.scope, key, expected, next)
    }

    /// CAS update may invoke `mutate` more than once after a concurrent write.
    /// Keep the closure free of external side effects.
    pub fn update<T, F>(&self, key: &str, mut mutate: F) -> Result<T, String>
    where
        T: Serialize + DeserializeOwned,
        F: FnMut(Option<T>) -> Result<T, String>,
    {
        loop {
            let current = self.get(key)?;
            let typed = current
                .as_ref()
                .map(|value| serde_json::from_value(value.clone()).map_err(|e| e.to_string()))
                .transpose()?;
            let next = mutate(typed)?;
            let value = serde_json::to_value(&next).map_err(|e| e.to_string())?;
            if self.compare_and_set(key, current, Some(value))? {
                return Ok(next);
            }
        }
    }
}
