//! Durable quarantine for imported-application removal.
//! The journal lets startup distinguish an interrupted removal from a
//! committed one without leaving a visible node whose asset has disappeared.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

fn removal_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Serialize, Deserialize)]
struct RemovalJournal {
    version: u32,
    node_id: String,
    asset_id: String,
    extension: String,
}

fn staging_root() -> Result<PathBuf, String> {
    Ok(dirs::home_dir()
        .ok_or("cannot locate home directory")?
        .join(".fiex")
        .join("application-staging"))
}

fn journal_path(staging: &Path, id: &str) -> PathBuf {
    staging.join(format!("remove-{id}.json"))
}

fn quarantine_path(staging: &Path, id: &str) -> PathBuf {
    staging.join(format!("remove-{id}.asset"))
}

fn save_journal(path: &Path, journal: &RemovalJournal) -> Result<(), String> {
    let parent = path.parent().ok_or("removal journal parent missing")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(journal).map_err(|error| error.to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    use std::io::Write;
    temporary
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

pub(super) fn remove(uri: &str) -> Result<(), String> {
    let id = app_id(uri).ok_or("not an Applications app node")?;
    let app = source(uri)?;
    let ApplicationSource::Imported {
        asset_id,
        extension,
    } = app.source
    else {
        return Err("System applications cannot be deleted in Fiex".into());
    };
    let asset = imported_asset_path(&asset_id, &extension)?;
    let _guard = removal_lock()
        .lock()
        .map_err(|_| "Applications removal lock poisoned")?;
    let staging = staging_root()?;
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let transaction_id = random_id()?;
    let journal = journal_path(&staging, &transaction_id);
    let quarantine = quarantine_path(&staging, &transaction_id);
    let record = RemovalJournal {
        version: 1,
        node_id: id.clone(),
        asset_id,
        extension,
    };
    save_journal(&journal, &record)?;

    if let Err(error) = std::fs::rename(&asset, &quarantine) {
        let _ = std::fs::remove_file(&journal);
        return Err(error.to_string());
    }
    if let Err(error) = update(|state| {
        let current = state
            .apps
            .get(&id)
            .ok_or_else(|| "NotFound: application".to_string())?;
        if !matches!(
            &current.source,
            ApplicationSource::Imported { asset_id: current_asset, .. }
                if current_asset == &record.asset_id
        ) {
            return Err("application asset changed during removal".into());
        }
        state.apps.remove(&id);
        Ok(())
    }) {
        let restored = std::fs::rename(&quarantine, &asset);
        if restored.is_ok() {
            let _ = std::fs::remove_file(&journal);
        }
        return Err(match restored {
            Ok(()) => error,
            Err(restore_error) => {
                format!("{error}; application removal recovery is pending: {restore_error}")
            }
        });
    }
    remove_node_if_present(&quarantine)?;
    std::fs::remove_file(&journal).map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn recover_pending() -> Result<(), String> {
    let _guard = removal_lock()
        .lock()
        .map_err(|_| "Applications removal lock poisoned")?;
    let staging = staging_root()?;
    let entries = match std::fs::read_dir(&staging) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let mut journals = entries
        .map(|entry| entry.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("remove-") && name.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    journals.sort();
    for path in journals {
        let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
        let record: RemovalJournal =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        if record.version != 1 || !valid_id(&record.node_id) || !valid_id(&record.asset_id) {
            return Err("invalid imported-application removal journal".into());
        }
        let asset = imported_asset_path(&record.asset_id, &record.extension)?;
        let transaction_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("remove-"))
            .and_then(|name| name.strip_suffix(".json"))
            .ok_or("invalid imported-application removal journal name")?;
        let quarantine = quarantine_path(&staging, transaction_id);
        let state = storage()
            .get(KEY)?
            .map(serde_json::from_value::<ApplicationsState>)
            .transpose()
            .map_err(|error| error.to_string())?;
        let Some(state) = state else {
            if quarantine.exists() && !asset.exists() {
                std::fs::rename(&quarantine, &asset).map_err(|error| error.to_string())?;
            } else if quarantine.exists() {
                remove_node_if_present(&quarantine)?;
            }
            std::fs::remove_file(path).map_err(|error| error.to_string())?;
            continue;
        };
        match state.apps.get(&record.node_id) {
            Some(node) if matches!(&node.source, ApplicationSource::Imported { asset_id, .. } if asset_id == &record.asset_id) => {
                if quarantine.exists() && !asset.exists() {
                    std::fs::rename(&quarantine, &asset).map_err(|error| error.to_string())?;
                } else if quarantine.exists() {
                    remove_node_if_present(&quarantine)?;
                } else if !asset.exists() {
                    return Err(format!(
                        "application asset missing during removal recovery: {}",
                        record.node_id
                    ));
                }
            }
            Some(_) => return Err("application removal journal no longer matches node".into()),
            None => {
                if state.apps.values().any(|node| {
                    matches!(&node.source, ApplicationSource::Imported { asset_id, .. } if asset_id == &record.asset_id)
                }) {
                    return Err("application asset is shared by another node".into());
                }
                remove_node_if_present(&quarantine)?;
                remove_node_if_present(&asset)?;
            }
        }
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn remove_node_if_present(path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let result = if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|error| error.to_string())
}
