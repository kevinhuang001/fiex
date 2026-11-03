//! Recent aliases persist stable source locators in the shared Core ProviderStorage owner.
//! Opening a mapped resource does not reuse the authorization from an earlier action.
use crate::{
    core::files::Entry,
    infrastructure::provider_storage::{native::NativeStorage, CoreModule},
    plugins::resource::resource_context::ResourceSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const LIMIT: usize = 40;
const SCOPE: &str = "history";
const RECORDS_KEY: &str = "items";
const LEGACY_MIGRATION_KEY: &str = "legacy-json-migrated";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RecentRecord {
    id: String,
    locator: String,
    name: String,
    extension: String,
    mime: String,
    size: Option<u64>,
    modified: Option<i64>,
    accessed_at: i64,
}

fn storage() -> crate::infrastructure::provider_storage::native::NativeScopedStorage {
    NativeStorage::new(CoreModule::Recent)
        .scoped(SCOPE)
        .expect("static Recent storage scope is valid")
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

fn locator_for(snapshot: &ResourceSnapshot) -> Result<String, String> {
    use crate::core::fs::provider_registry::ResourceOwner;
    if snapshot.display.starts_with("recent://") {
        return source_for(&snapshot.display);
    }
    let owner =
        crate::core::fs::provider_registry::owner(&snapshot.display).map_err(String::from)?;
    if !matches!(
        owner,
        ResourceOwner::LocalFs
            | ResourceOwner::PluginMount { .. }
            | ResourceOwner::NativeVfs("core:usb-fs")
    ) {
        return Err("resource cannot be saved in Recent".into());
    }
    if matches!(owner, ResourceOwner::LocalFs) {
        std::fs::canonicalize(&snapshot.display)
            .map(|path| path.to_string_lossy().into_owned())
            .or_else(|_| Ok(snapshot.display.clone()))
    } else {
        Ok(snapshot.display.clone())
    }
}

fn snapshot_record(locator: String, snapshot: &ResourceSnapshot) -> Result<RecentRecord, String> {
    if snapshot.is_dir {
        return Err("Recent does not record directories".into());
    }
    let metadata = snapshot.metadata().cloned().unwrap_or_default();
    Ok(RecentRecord {
        id: opaque_id()?,
        locator,
        name: metadata.name,
        extension: metadata.extension.unwrap_or_default(),
        mime: metadata.mime.unwrap_or_default(),
        size: metadata.size,
        modified: metadata.modified,
        accessed_at: now(),
    })
}

fn migrate_legacy_json() -> Result<(), String> {
    let store = storage();
    if store.get(LEGACY_MIGRATION_KEY)?.is_some() {
        return Ok(());
    }
    let Some(file) = dirs::home_dir().map(|home| home.join(".fiex").join("recent.json")) else {
        store.set(LEGACY_MIGRATION_KEY, serde_json::Value::Bool(true))?;
        return Ok(());
    };
    let legacy = std::fs::read(&file).ok();
    let paths = legacy
        .as_deref()
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(bytes).ok())
        .unwrap_or_default();
    let mut records = store
        .get(RECORDS_KEY)?
        .map(serde_json::from_value::<Vec<RecentRecord>>)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let mut seen: HashSet<String> = records
        .iter()
        .map(|record| record.locator.clone())
        .collect();
    for path in paths.into_iter().take(LIMIT) {
        if seen.contains(&path) {
            continue;
        }
        let Ok(snapshot) = ResourceSnapshot::resolve(&path) else {
            continue;
        };
        if let Ok(record) = snapshot_record(path.clone(), &snapshot) {
            seen.insert(path);
            records.push(record);
        }
    }
    records.sort_by_key(|record| std::cmp::Reverse(record.accessed_at));
    records.truncate(LIMIT);
    store.set(
        RECORDS_KEY,
        serde_json::to_value(records).map_err(|error| error.to_string())?,
    )?;
    if let Some(bytes) = legacy {
        let backup = file.with_extension("json.migrated-backup");
        if !backup.exists() {
            std::fs::write(&backup, bytes).map_err(|error| error.to_string())?;
        }
        std::fs::remove_file(&file).map_err(|error| error.to_string())?;
    }
    store.set(LEGACY_MIGRATION_KEY, serde_json::Value::Bool(true))
}

fn records() -> Result<Vec<RecentRecord>, String> {
    migrate_legacy_json()?;
    storage()
        .get(RECORDS_KEY)?
        .map(serde_json::from_value)
        .transpose()
        .map(|records| records.unwrap_or_default())
        .map_err(|error| error.to_string())
}

/// Called by Core's successful-open hook. The snapshot has already been
/// validated by Core; only its stable locator and display metadata are saved.
pub fn record_opened(snapshot: &ResourceSnapshot) -> Result<(), String> {
    let locator = locator_for(snapshot)?;
    if locator.starts_with("applications://") {
        return Ok(());
    }
    let mut updated = snapshot_record(locator.clone(), snapshot)?;
    let store = storage();
    store.update(RECORDS_KEY, |current: Option<Vec<RecentRecord>>| {
        let mut items = current.unwrap_or_default();
        if let Some(existing) = items.iter_mut().find(|item| item.locator == locator) {
            updated.id = existing.id.clone();
            *existing = updated.clone();
            items.sort_by_key(|record| std::cmp::Reverse(record.accessed_at));
        } else {
            items.insert(0, updated.clone());
        }
        items.truncate(LIMIT);
        Ok(items)
    })?;
    Ok(())
}

/// Compatibility entry point for successful desktop opens. The `Recent`
/// handle carries no private cache; Hook and VFS always consult ProviderStorage.
pub fn source_for(alias: &str) -> Result<String, String> {
    let id = alias
        .strip_prefix("recent://")
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .ok_or("not a recent item")?;
    records()?
        .into_iter()
        .find(|record| record.id == id)
        .map(|record| record.locator)
        .ok_or_else(|| "StaleRef: item is no longer in Recent".into())
}

fn record_for_alias(alias: &str) -> Result<RecentRecord, String> {
    let id = alias
        .strip_prefix("recent://")
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .ok_or("not a recent item")?;
    records()?
        .into_iter()
        .find(|record| record.id == id)
        .ok_or_else(|| "StaleRef: item is no longer in Recent".into())
}

fn entry_for_record(record: &RecentRecord) -> Entry {
    Entry {
        path: format!("recent://{}", record.id),
        name: record.name.clone(),
        is_dir: false,
        extension: record.extension.clone(),
        mime: record.mime.clone(),
        size: record.size.unwrap_or_default(),
        modified: record.modified.unwrap_or_default().max(0) as u64,
        created: record.accessed_at.max(0) as u64,
        allowed_ops: vec!["stat".into(), "read".into()],
        ..Default::default()
    }
}

pub fn entry_for_alias(alias: &str) -> Result<Entry, String> {
    Ok(entry_for_record(&record_for_alias(alias)?))
}

pub fn entries() -> Vec<Entry> {
    records()
        .unwrap_or_default()
        .iter()
        .map(entry_for_record)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_aliases_are_stable_opaque_ids() {
        let id = opaque_id().unwrap();
        let alias = format!("recent://{id}");
        assert!(alias.starts_with("recent://"));
        assert!(!id.contains('/'));
        assert!(!id.contains(':'));
        assert_eq!(alias, format!("recent://{id}"));
        assert!(source_for("recent://unregistered-identity").is_err());
    }
}
