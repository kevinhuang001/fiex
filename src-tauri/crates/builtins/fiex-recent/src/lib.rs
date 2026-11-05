//! Recent module persistence and alias behavior.
//!
//! Filesystem discovery, authorization, UI projection, and provider registration
//! are supplied by the application adapter. This crate owns the stable records,
//! alias validation, storage updates, and legacy storage import.

use fiex_storage::native::NativeScopedStorage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

mod module;

pub const PROVIDER_ID: &str = "core:recent";

pub use module::{
    recent_uri_schemes, Existence, RecentContextMenu, RecentHook, RecentModule,
    RecentModuleServices, RecentResource, RecentVfs,
};

const LIMIT: usize = 40;
const RECORDS_KEY: &str = "items";
const LEGACY_MIGRATION_KEY: &str = "legacy-json-migrated";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecentRecord {
    pub id: String,
    pub locator: String,
    pub name: String,
    pub extension: String,
    pub mime: String,
    pub size: Option<u64>,
    pub modified: Option<i64>,
    pub accessed_at: i64,
}

impl RecentRecord {
    pub fn new(
        locator: impl Into<String>,
        name: impl Into<String>,
        extension: impl Into<String>,
        mime: impl Into<String>,
        size: Option<u64>,
        modified: Option<i64>,
    ) -> Result<Self, String> {
        Ok(Self {
            id: opaque_id()?,
            locator: locator.into(),
            name: name.into(),
            extension: extension.into(),
            mime: mime.into(),
            size,
            modified,
            accessed_at: now(),
        })
    }
}

#[derive(Clone)]
pub struct RecentStore {
    storage: NativeScopedStorage,
}

impl RecentStore {
    pub fn new(storage: NativeScopedStorage) -> Self {
        Self { storage }
    }

    pub fn ensure_legacy_migrated(
        &self,
        mut resolve_legacy: impl FnMut(&str) -> Option<RecentRecord>,
    ) -> Result<(), String> {
        if self.storage.get(LEGACY_MIGRATION_KEY)?.is_some() {
            return Ok(());
        }
        let Some(file) = dirs::home_dir().map(|home| home.join(".fiex").join("recent.json")) else {
            self.storage
                .set(LEGACY_MIGRATION_KEY, serde_json::Value::Bool(true))?;
            return Ok(());
        };
        let legacy = std::fs::read(&file).ok();
        let paths = legacy
            .as_deref()
            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(bytes).ok())
            .unwrap_or_default();
        let mut records = self.records()?;
        let mut seen: HashSet<String> = records
            .iter()
            .map(|record| record.locator.clone())
            .collect();
        for path in paths.into_iter().take(LIMIT) {
            if seen.contains(&path) {
                continue;
            }
            if let Some(record) = resolve_legacy(&path) {
                seen.insert(path);
                records.push(record);
            }
        }
        records.sort_by_key(|record| std::cmp::Reverse(record.accessed_at));
        records.truncate(LIMIT);
        self.save_records(records)?;
        if let Some(bytes) = legacy {
            let backup = file.with_extension("json.migrated-backup");
            if !backup.exists() {
                std::fs::write(&backup, bytes).map_err(|error| error.to_string())?;
            }
            std::fs::remove_file(&file).map_err(|error| error.to_string())?;
        }
        self.storage
            .set(LEGACY_MIGRATION_KEY, serde_json::Value::Bool(true))
    }

    pub fn records(&self) -> Result<Vec<RecentRecord>, String> {
        self.storage
            .get(RECORDS_KEY)?
            .map(serde_json::from_value)
            .transpose()
            .map(|records| records.unwrap_or_default())
            .map_err(|error| error.to_string())
    }

    pub fn upsert(&self, mut updated: RecentRecord) -> Result<(), String> {
        self.storage
            .update(RECORDS_KEY, |current: Option<Vec<RecentRecord>>| {
                let mut records = current.unwrap_or_default();
                if let Some(existing) = records
                    .iter_mut()
                    .find(|record| record.locator == updated.locator)
                {
                    updated.id.clone_from(&existing.id);
                    *existing = updated.clone();
                    records.sort_by_key(|record| std::cmp::Reverse(record.accessed_at));
                } else {
                    records.insert(0, updated.clone());
                }
                records.truncate(LIMIT);
                Ok(records)
            })?;
        Ok(())
    }

    pub fn record_for_alias(&self, alias: &str) -> Result<RecentRecord, String> {
        let id = alias_id(alias)?;
        self.records()?
            .into_iter()
            .find(|record| record.id == id)
            .ok_or_else(|| "StaleRef: item is no longer in Recent".into())
    }

    pub fn source_for(&self, alias: &str) -> Result<String, String> {
        self.record_for_alias(alias).map(|record| record.locator)
    }

    pub fn remove_aliases(&self, aliases: &[String]) -> Result<Vec<String>, String> {
        let ids = aliases
            .iter()
            .map(|alias| alias_id(alias).map(str::to_owned))
            .collect::<Result<HashSet<_>, _>>()?;
        if ids.is_empty() || ids.len() != aliases.len() {
            return Err("invalid Recent selection".into());
        }
        self.storage
            .update(RECORDS_KEY, |current: Option<Vec<RecentRecord>>| {
                let mut records = current.unwrap_or_default();
                records.retain(|record| !ids.contains(&record.id));
                Ok(records)
            })?;
        let remaining = self.records()?;
        let remaining_ids = remaining
            .iter()
            .map(|record| record.id.as_str())
            .collect::<HashSet<_>>();
        Ok(aliases
            .iter()
            .filter(|alias| alias_id(alias).is_ok_and(|id| !remaining_ids.contains(id)))
            .cloned()
            .collect())
    }

    fn save_records(&self, records: Vec<RecentRecord>) -> Result<(), String> {
        self.storage.set(
            RECORDS_KEY,
            serde_json::to_value(records).map_err(|error| error.to_string())?,
        )
    }
}

fn alias_id(alias: &str) -> Result<&str, String> {
    alias
        .strip_prefix("recent://")
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .ok_or_else(|| "not a recent item".into())
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_secs() as i64)
        .unwrap_or_default()
}

fn opaque_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|error| error.to_string())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let value = hex::encode(bytes);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &value[..8],
        &value[8..12],
        &value[12..16],
        &value[16..20],
        &value[20..]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fiex_storage::{native::OwnerBoundStorage, CoreOwnerKey};

    fn store() -> RecentStore {
        let scope = format!("test-{}", opaque_id().unwrap());
        RecentStore::new(
            OwnerBoundStorage::from_registry(CoreOwnerKey::new("fiex-recent-test"))
                .scoped(&scope)
                .unwrap(),
        )
    }

    fn record(locator: &str, name: &str) -> RecentRecord {
        RecentRecord::new(locator, name, "txt", "text/plain", Some(4), None).unwrap()
    }

    #[test]
    fn upsert_keeps_the_alias_for_an_existing_locator() {
        let store = store();
        let first = record("/tmp/a.txt", "a.txt");
        let id = first.id.clone();
        store.upsert(first).unwrap();
        store.upsert(record("/tmp/a.txt", "renamed.txt")).unwrap();
        let saved = store.records().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, id);
        assert_eq!(saved[0].name, "renamed.txt");
    }

    #[test]
    fn removing_aliases_only_removes_selected_records() {
        let store = store();
        let first = record("/tmp/a.txt", "a.txt");
        let second = record("/tmp/b.txt", "b.txt");
        let third = record("/tmp/c.txt", "c.txt");
        let selected = vec![
            format!("recent://{}", first.id),
            format!("recent://{}", second.id),
        ];
        for value in [first, second, third.clone()] {
            store.upsert(value).unwrap();
        }
        assert_eq!(store.remove_aliases(&selected).unwrap(), selected);
        assert_eq!(store.records().unwrap(), vec![third]);
    }

    #[test]
    fn invalid_and_stale_aliases_fail_closed() {
        let store = store();
        assert!(store.source_for("recent://").is_err());
        assert!(store.source_for("recent://abc/child").is_err());
        assert!(store.source_for("recent://not-registered").is_err());
        assert!(store
            .remove_aliases(&["recent://ok".into(), "bad".into()])
            .is_err());
    }
}
