//! Recent aliases persist stable source locators in the shared Core ProviderStorage owner.
//! Opening a mapped resource does not reuse the authorization from an earlier action.
use crate::{core::files::Entry, plugins::resource::resource_context::ResourceSnapshot};
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
    crate::core::volumes::native_vfs::management_storage_for_provider("core:recent", SCOPE)
        .expect("Recent provider registration binds its storage owner")
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

pub(crate) type RecentStorage =
    crate::infrastructure::provider_storage::native::NativeScopedStorage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Existence {
    Present,
    Missing,
    Unknown,
}

/// A deliberately narrow metadata probe. Only an unambiguous not-found result
/// removes a row from Recent; provider and permission failures remain Unknown.
fn probe_existence(locator: &str) -> Existence {
    use crate::core::fs::provider_registry::ResourceOwner;
    match crate::core::fs::provider_registry::owner(locator) {
        Ok(ResourceOwner::LocalFs) => match std::fs::symlink_metadata(locator) {
            Ok(_) => Existence::Present,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Existence::Missing,
            Err(_) => Existence::Unknown,
        },
        Ok(ResourceOwner::NativeVfs("core:usb-fs")) => {
            let Some(route) = crate::usbfs::route(locator) else {
                return Existence::Unknown;
            };
            match crate::usbfs::entry(&route) {
                Ok(_) => Existence::Present,
                Err(error) if error.contains("NotFound") => Existence::Missing,
                Err(_) => Existence::Unknown,
            }
        }
        // Plugin mounts do not expose a metadata-only existence capability.
        // Do not fall back to stat, which can have different authorization
        // semantics. Core VFS types without a dedicated probe are Unknown too.
        Ok(ResourceOwner::PluginMount { .. }) | Ok(ResourceOwner::NativeVfs(_)) | Err(_) => {
            Existence::Unknown
        }
    }
}

fn record_is_visible(record: &RecentRecord) -> bool {
    probe_existence(&record.locator) != Existence::Missing
}

fn visible_records(records: Vec<RecentRecord>) -> Vec<RecentRecord> {
    records.into_iter().filter(record_is_visible).collect()
}

fn locator_for(snapshot: &ResourceSnapshot, store: &RecentStorage) -> Result<String, String> {
    use crate::core::fs::provider_registry::ResourceOwner;
    if snapshot.display.starts_with("recent://") {
        return source_for_with_storage(&snapshot.display, store);
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

fn migrate_legacy_json(store: &RecentStorage) -> Result<(), String> {
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

fn records_with_storage(store: &RecentStorage) -> Result<Vec<RecentRecord>, String> {
    migrate_legacy_json(store)?;
    store
        .get(RECORDS_KEY)?
        .map(serde_json::from_value)
        .transpose()
        .map(|records| records.unwrap_or_default())
        .map_err(|error| error.to_string())
}

/// Called by Core's successful-open hook. The snapshot has already been
/// validated by Core; only its stable locator and display metadata are saved.
pub(crate) fn record_opened(
    snapshot: &ResourceSnapshot,
    store: &crate::infrastructure::provider_storage::native::NativeScopedStorage,
) -> Result<(), String> {
    let locator = locator_for(snapshot, store)?;
    if locator.starts_with("applications://") {
        return Ok(());
    }
    let mut updated = snapshot_record(locator.clone(), snapshot)?;
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

/// Resolve a Recent alias through the same ProviderStorage owner used by the
/// Hook and native VFS registry.
pub fn source_for(alias: &str) -> Result<String, String> {
    source_for_with_storage(alias, &storage())
}

pub(crate) fn source_for_with_storage(
    alias: &str,
    store: &RecentStorage,
) -> Result<String, String> {
    let id = alias
        .strip_prefix("recent://")
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .ok_or("not a recent item")?;
    records_with_storage(store)?
        .into_iter()
        .find(|record| record.id == id)
        .map(|record| record.locator)
        .ok_or_else(|| "StaleRef: item is no longer in Recent".into())
}

fn record_for_alias_with_storage(
    alias: &str,
    store: &RecentStorage,
) -> Result<RecentRecord, String> {
    let id = alias
        .strip_prefix("recent://")
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .ok_or("not a recent item")?;
    records_with_storage(store)?
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
        storage_projection: crate::core::files::StorageProjection::Projected(
            crate::plugins::match_rules::projected_storage_for_source(&record.locator),
        ),
        ..Default::default()
    }
}

pub fn entry_for_alias(alias: &str) -> Result<Entry, String> {
    entry_for_alias_with_storage(alias, &storage())
}

pub(crate) fn entry_for_alias_with_storage(
    alias: &str,
    store: &RecentStorage,
) -> Result<Entry, String> {
    Ok(entry_for_record(&record_for_alias_with_storage(
        alias, store,
    )?))
}

pub fn entries() -> Vec<Entry> {
    entries_with_storage(&storage()).unwrap_or_default()
}

pub(crate) fn entries_with_storage(store: &RecentStorage) -> Result<Vec<Entry>, String> {
    Ok(visible_records(records_with_storage(store)?)
        .iter()
        .map(entry_for_record)
        .collect())
}

/// Filter a cached Recent snapshot before it can be painted. This preserves
/// Unknown/offline rows while preventing a cached local path known to be
/// missing from flashing back into the view before the next live listing.
pub(crate) fn filter_cached_entries(
    entries: &mut Vec<Entry>,
    store: &RecentStorage,
) -> Result<(), String> {
    let records = records_with_storage(store)?;
    let by_alias = records
        .into_iter()
        .map(|record| (format!("recent://{}", record.id), record))
        .collect::<std::collections::HashMap<_, _>>();
    entries.retain(|entry| by_alias.get(&entry.path).is_some_and(record_is_visible));
    Ok(())
}

pub(crate) fn remove_aliases_with_storage(
    aliases: &[String],
    store: &RecentStorage,
) -> Result<Vec<String>, String> {
    let ids = aliases
        .iter()
        .filter_map(|alias| {
            alias
                .strip_prefix("recent://")
                .filter(|id| !id.is_empty() && !id.contains('/'))
                .map(str::to_owned)
        })
        .collect::<HashSet<_>>();
    if ids.is_empty() || ids.len() != aliases.len() {
        return Err("invalid Recent selection".into());
    }
    store.update(RECORDS_KEY, |current: Option<Vec<RecentRecord>>| {
        let mut records = current.unwrap_or_default();
        records.retain(|record| !ids.contains(&record.id));
        Ok(records)
    })?;
    // Re-read the committed record set to distinguish selected aliases that
    // were actually removed. The operation is idempotent if one is already
    // absent; both cache and mutation overlay still discard that stale row.
    let remaining = records_with_storage(store)?;
    let remaining_ids = remaining
        .iter()
        .map(|record| record.id.as_str())
        .collect::<HashSet<_>>();
    let affected = aliases
        .iter()
        .filter(|alias| {
            alias
                .strip_prefix("recent://")
                .is_some_and(|id| !remaining_ids.contains(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    for alias in &affected {
        crate::core::fs::changes::removed_from(alias, "recent://");
    }
    Ok(affected)
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

    #[test]
    fn local_existence_probe_distinguishes_missing_and_present_without_following_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("recent.txt");
        std::fs::write(&file, "contents").unwrap();
        assert_eq!(probe_existence(file.to_str().unwrap()), Existence::Present);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(probe_existence(file.to_str().unwrap()), Existence::Missing);
    }

    #[test]
    fn remove_aliases_changes_only_the_selected_storage_records() {
        let store = crate::core::volumes::native_vfs::management_storage_for_provider(
            "core:recent",
            "remove-test",
        )
        .unwrap();
        let ids = [
            opaque_id().unwrap(),
            opaque_id().unwrap(),
            opaque_id().unwrap(),
        ];
        let records = ids
            .iter()
            .enumerate()
            .map(|(index, id)| RecentRecord {
                id: id.clone(),
                locator: format!("/fiex-recent-test/{index}.txt"),
                name: format!("{index}.txt"),
                extension: "txt".into(),
                mime: "text/plain".into(),
                size: Some(1),
                modified: None,
                accessed_at: index as i64,
            })
            .collect::<Vec<_>>();
        store
            .set(RECORDS_KEY, serde_json::to_value(&records).unwrap())
            .unwrap();

        let selected = vec![
            format!("recent://{}", ids[0]),
            format!("recent://{}", ids[1]),
        ];
        assert_eq!(
            remove_aliases_with_storage(&selected, &store).unwrap(),
            selected
        );
        assert!(source_for_with_storage(&format!("recent://{}", ids[0]), &store).is_err());
        assert!(source_for_with_storage(&format!("recent://{}", ids[1]), &store).is_err());
        assert_eq!(
            source_for_with_storage(&format!("recent://{}", ids[2]), &store).unwrap(),
            "/fiex-recent-test/2.txt"
        );

        store.delete(RECORDS_KEY).unwrap();
        store.delete(LEGACY_MIGRATION_KEY).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn broken_local_symlink_is_present_in_recent() {
        let directory = tempfile::tempdir().unwrap();
        let link = directory.path().join("broken-link");
        std::os::unix::fs::symlink(directory.path().join("missing"), &link).unwrap();
        assert_eq!(probe_existence(link.to_str().unwrap()), Existence::Present);
    }
}
