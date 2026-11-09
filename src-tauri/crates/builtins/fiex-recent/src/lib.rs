//! Recent module persistence and alias behavior.
//!
//! Filesystem discovery, authorization, UI projection, and provider registration
//! are supplied by the application adapter. This crate owns the stable records,
//! alias validation and storage updates.

use fiex_storage::native::NativeScopedStorage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

mod module;

pub const PROVIDER_ID: &str = "core:recent";

pub use module::{
    Existence, RecentContextMenu, RecentHook, RecentModule, RecentModuleServices, RecentResource,
    RecentVfs, recent_uri_schemes,
};

const LIMIT: usize = 40;
const RECORDS_KEY: &str = "items";

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
        let record = self
            .records()?
            .into_iter()
            .find(|record| record.id == id)
            .ok_or_else(|| "StaleRef: item is no longer in Recent".to_owned())?;
        if recent_uri(&record).as_str() != alias {
            return Err("StaleRef: item is no longer in Recent".into());
        }
        Ok(record)
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
                for alias in aliases {
                    let id = alias_id(alias)?;
                    let record = records
                        .iter()
                        .find(|record| record.id == id)
                        .ok_or_else(|| "StaleRef: item is no longer in Recent".to_owned())?;
                    if recent_uri(record).as_str() != alias {
                        return Err("StaleRef: item is no longer in Recent".into());
                    }
                }
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
}

fn alias_id(alias: &str) -> Result<&str, String> {
    let (path, query) = alias
        .strip_prefix("recent://")
        .and_then(|value| value.split_once('?'))
        .ok_or_else(|| "not a recent item".to_owned())?;
    let id = query
        .strip_prefix("id=")
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "not a recent item".to_owned())?;
    if path.is_empty()
        || path.contains('/')
        || id.contains('&')
        || id.contains('/')
        || id.contains('?')
    {
        return Err("not a recent item".into());
    }
    Ok(id)
}

fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            encoded.push(char::from(b"0123456789ABCDEF"[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

pub fn recent_uri(record: &RecentRecord) -> String {
    format!("recent://{}?id={}", encode_segment(&record.name), record.id)
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
    use fiex_storage::{CoreOwnerKey, native::OwnerBoundStorage};

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
        let selected = vec![recent_uri(&first), recent_uri(&second)];
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
        assert!(
            store
                .remove_aliases(&["recent://ok".into(), "bad".into()])
                .is_err()
        );
    }
}
