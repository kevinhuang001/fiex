//! Application adapters for the Recent module's storage and source services.
use crate::{core::files::Entry, plugins::resource::resource_context::ResourceSnapshot};
use fiex_recent::{RecentRecord, RecentStore};

const SCOPE: &str = "history";

fn storage() -> fiex_storage::native::NativeScopedStorage {
    crate::core::volumes::native_vfs::management_storage_for_provider("core:recent", SCOPE)
        .expect("Recent provider registration binds its storage owner")
}

pub(crate) type RecentStorage = fiex_storage::native::NativeScopedStorage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Existence {
    Present,
    Missing,
    Unknown,
}

/// Only an unambiguous not-found result removes a row; permission and provider
/// failures remain Unknown so an offline resource does not disappear.
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

fn locator_for(snapshot: &ResourceSnapshot, recent: &RecentStore) -> Result<String, String> {
    if snapshot.display.starts_with("recent://") {
        return recent.source_for(&snapshot.display);
    }
    use crate::core::fs::provider_registry::ResourceOwner;
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
    RecentRecord::new(
        locator,
        metadata.name,
        metadata.extension.unwrap_or_default(),
        metadata.mime.unwrap_or_default(),
        metadata.size,
        metadata.modified,
    )
}

fn recent_store(storage: &RecentStorage) -> RecentStore {
    RecentStore::new(storage.clone())
}

fn ensure_migrated(recent: &RecentStore) -> Result<(), String> {
    recent.ensure_legacy_migrated(|path| {
        let snapshot = crate::plugins::resource::resource_context::resolve(path).ok()?;
        snapshot_record(path.to_owned(), &snapshot).ok()
    })
}

fn records_with_storage(storage: &RecentStorage) -> Result<Vec<RecentRecord>, String> {
    let recent = recent_store(storage);
    ensure_migrated(&recent)?;
    recent.records()
}

/// Called by Core's successful-open hook after the source snapshot has been
/// validated. The module stores only a locator and display metadata.
pub(crate) fn record_opened(
    snapshot: &ResourceSnapshot,
    storage: &RecentStorage,
) -> Result<(), String> {
    let recent = recent_store(storage);
    ensure_migrated(&recent)?;
    let locator = locator_for(snapshot, &recent)?;
    if locator.starts_with("applications://") {
        return Ok(());
    }
    recent.upsert(snapshot_record(locator, snapshot)?)
}

pub fn source_for(alias: &str) -> Result<String, String> {
    source_for_with_storage(alias, &storage())
}

pub(crate) fn source_for_with_storage(
    alias: &str,
    storage: &RecentStorage,
) -> Result<String, String> {
    let recent = recent_store(storage);
    ensure_migrated(&recent)?;
    recent.source_for(alias)
}

fn record_for_alias_with_storage(
    alias: &str,
    storage: &RecentStorage,
) -> Result<RecentRecord, String> {
    let recent = recent_store(storage);
    ensure_migrated(&recent)?;
    recent.record_for_alias(alias)
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
        storage_projection: fiex_resource::StorageProjection::Projected(
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
    storage: &RecentStorage,
) -> Result<Entry, String> {
    Ok(entry_for_record(&record_for_alias_with_storage(
        alias, storage,
    )?))
}

pub fn entries() -> Vec<Entry> {
    entries_with_storage(&storage()).unwrap_or_default()
}

pub(crate) fn entries_with_storage(storage: &RecentStorage) -> Result<Vec<Entry>, String> {
    Ok(visible_records(records_with_storage(storage)?)
        .iter()
        .map(entry_for_record)
        .collect())
}

/// Filter a cached Recent snapshot before it can be painted. Unknown probes
/// preserve offline rows; only known-missing sources are hidden.
pub(crate) fn filter_cached_entries(
    entries: &mut Vec<Entry>,
    storage: &RecentStorage,
) -> Result<(), String> {
    let by_alias = records_with_storage(storage)?
        .into_iter()
        .map(|record| (format!("recent://{}", record.id), record))
        .collect::<std::collections::HashMap<_, _>>();
    entries.retain(|entry| by_alias.get(&entry.path).is_some_and(record_is_visible));
    Ok(())
}

pub(crate) fn remove_aliases_with_storage(
    aliases: &[String],
    storage: &RecentStorage,
) -> Result<Vec<String>, String> {
    let recent = recent_store(storage);
    ensure_migrated(&recent)?;
    let affected = recent.remove_aliases(aliases)?;
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
        let record = RecentRecord::new(
            "/fiex-recent-test/a.txt",
            "a.txt",
            "txt",
            "text/plain",
            Some(1),
            None,
        )
        .unwrap();
        assert!(!record.id.contains('/'));
        assert!(!record.id.contains(':'));
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
        let storage = crate::core::volumes::native_vfs::management_storage_for_provider(
            "core:recent",
            "remove-test",
        )
        .unwrap();
        let recent = RecentStore::new(storage.clone());
        let records = (0..3)
            .map(|index| {
                RecentRecord::new(
                    format!("/fiex-recent-test/{index}.txt"),
                    format!("{index}.txt"),
                    "txt",
                    "text/plain",
                    Some(1),
                    None,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let ids = records
            .iter()
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        storage
            .set("items", serde_json::to_value(&records).unwrap())
            .unwrap();
        let selected = vec![
            format!("recent://{}", ids[0]),
            format!("recent://{}", ids[1]),
        ];
        assert_eq!(recent.remove_aliases(&selected).unwrap(), selected);
        assert!(recent.source_for(&format!("recent://{}", ids[0])).is_err());
        assert!(recent.source_for(&format!("recent://{}", ids[1])).is_err());
        assert_eq!(
            recent.source_for(&format!("recent://{}", ids[2])).unwrap(),
            "/fiex-recent-test/2.txt"
        );
        storage.delete("items").unwrap();
        storage.delete("legacy-json-migrated").unwrap();
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
