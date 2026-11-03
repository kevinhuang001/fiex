//! Native Adapter for trusted Core Modules. This is a direct function adapter
//! into the same storage Engine used by the Plugin IPC adapter.
use super::{
    engine,
    owner::{CoreModule, OwnerId},
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

#[derive(Clone)]
pub(crate) struct NativeStorage {
    owner: OwnerId,
}

impl NativeStorage {
    pub(crate) const fn new(module: CoreModule) -> Self {
        Self {
            owner: OwnerId::core(module),
        }
    }

    pub(crate) fn scoped(&self, scope: &str) -> Result<NativeScopedStorage, String> {
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
pub(crate) struct NativeScopedStorage {
    owner: OwnerId,
    scope: String,
}

impl NativeScopedStorage {
    pub(crate) fn get(&self, key: &str) -> Result<Option<Value>, String> {
        engine::get(&self.owner, &self.scope, key)
    }

    pub(crate) fn set(&self, key: &str, value: Value) -> Result<(), String> {
        engine::set(&self.owner, &self.scope, key, value)
    }

    #[allow(dead_code)] // Parity with the plugin Storage adapter.
    pub(crate) fn delete(&self, key: &str) -> Result<(), String> {
        engine::delete(&self.owner, &self.scope, key)
    }

    #[allow(dead_code)] // Parity with the plugin Storage adapter.
    pub(crate) fn keys(&self) -> Result<Vec<String>, String> {
        engine::list_keys(&self.owner, &self.scope)
    }

    pub(crate) fn compare_and_set(
        &self,
        key: &str,
        expected: Option<Value>,
        next: Option<Value>,
    ) -> Result<bool, String> {
        engine::compare_and_set(&self.owner, &self.scope, key, expected, next)
    }

    /// CAS update may invoke `mutate` more than once after a concurrent write.
    /// Keep the closure free of external side effects.
    pub(crate) fn update<T, F>(&self, key: &str, mut mutate: F) -> Result<T, String>
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
