//! Builds and validates a journal for converting the legacy Applications tree.
use super::*;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
#[path = "scan.rs"]
mod scan;

#[derive(Default, Deserialize)]
struct LegacyFolderStore {
    #[serde(default)]
    folders: std::collections::BTreeMap<String, Vec<String>>,
}

pub(super) fn legacy_plan_exists() -> Result<bool, String> {
    let root = app_root()?;
    let has_tree = std::fs::read_dir(&root)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some();
    Ok(has_tree || legacy_folder_path()?.is_file())
}

fn decode_legacy_folder(uri: &str) -> Result<Option<String>, String> {
    if uri == crate::core::volumes::applications::URI {
        return Ok(None);
    }
    let encoded = uri
        .strip_prefix("applications://~")
        .ok_or("invalid legacy Applications folder URI")?;
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|error| error.to_string())?;
    let relative = String::from_utf8(bytes).map_err(|error| error.to_string())?;
    let normalized = relative.replace('\\', "/");
    super::safe_relative(&normalized)?;
    Ok(Some(normalized))
}

pub(super) fn build_plan() -> Result<MigrationPlan, String> {
    let root = app_root()?;
    let migration_id = super::super::random_id()?;
    let mut state = ApplicationsState {
        schema: SCHEMA,
        revision: 1,
        migration_id: Some(migration_id.clone()),
        ..Default::default()
    };
    let mut assets = Vec::new();
    let mut recovery = Vec::new();
    let mut directories = Vec::new();
    let mut warnings = Vec::new();
    let mut folder_by_relative = HashMap::new();
    folder_by_relative.insert(String::new(), None::<String>);
    if root.is_dir() {
        scan::scan_directory(
            &root,
            Path::new(""),
            None,
            &mut state,
            &mut folder_by_relative,
            &mut assets,
            &mut recovery,
            &mut directories,
            &mut warnings,
        )?;
    }
    let folder_path = legacy_folder_path()?;
    let has_legacy_folder_map = folder_path.is_file();
    if has_legacy_folder_map {
        let bytes = std::fs::read(&folder_path).map_err(|error| error.to_string())?;
        let map: LegacyFolderStore = serde_json::from_slice(&bytes)
            .map_err(|error| format!("legacy Applications mapping is corrupted: {error}"))?;
        let system_entries =
            crate::core::volumes::applications::platform::platform_list().unwrap_or_default();
        let mut by_old_uri = HashMap::new();
        for entry in system_entries {
            let encoded = entry
                .path
                .strip_prefix(crate::core::volumes::applications::URI)
                .ok_or("system application has invalid legacy URI")?;
            let locator = crate::core::volumes::applications::decode(encoded)?;
            by_old_uri.insert(entry.path, (locator, entry.name));
        }
        let mut assigned = HashSet::new();
        for (folder_uri, old_apps) in map.folders {
            let relative = decode_legacy_folder(&folder_uri)?;
            let parent_id = relative
                .as_deref()
                .map(|path| scan::ensure_folder_path(path, &mut state, &mut folder_by_relative))
                .transpose()?;
            for old_app in old_apps {
                let (locator, display_name, unavailable) = if let Some((locator, display_name)) =
                    by_old_uri.get(&old_app)
                {
                    (locator.clone(), display_name.clone(), false)
                } else if let Some(encoded) =
                    old_app.strip_prefix(crate::core::volumes::applications::URI)
                {
                    match crate::core::volumes::applications::decode(encoded) {
                        Ok(locator) if !locator.is_empty() => {
                            let fallback = scan::fallback_system_name(&locator);
                            (locator, fallback, true)
                        }
                        _ => {
                            warnings
                                .push(format!("unresolved system application mapping: {old_app}"));
                            continue;
                        }
                    }
                } else {
                    warnings.push(format!("unresolved system application mapping: {old_app}"));
                    continue;
                };
                let id = fiex_applications::system_id(&locator);
                if !assigned.insert(id.clone()) {
                    warnings.push(format!("duplicate system application mapping: {old_app}"));
                    continue;
                }
                let order = super::super::next_order(&state, parent_id.as_deref());
                state.apps.insert(
                    id.clone(),
                    ApplicationNode {
                        id,
                        parent: parent_id.clone(),
                        display_name,
                        source: ApplicationSource::System {
                            stable_locator: locator,
                        },
                        order,
                        unavailable,
                    },
                );
            }
        }
    }
    super::validate_state(&state)?;
    Ok(MigrationPlan {
        version: 1,
        migration_id,
        committed: false,
        state,
        assets,
        recovery,
        directories,
        has_legacy_folder_map,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(value: &str) -> String {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value)
    }

    #[test]
    fn legacy_folder_uris_migrate_only_confined_relative_paths() {
        assert_eq!(
            decode_legacy_folder(crate::core::volumes::applications::URI).unwrap(),
            None
        );
        assert_eq!(
            decode_legacy_folder(&format!("applications://~{}", encoded("Work/Editor")))
                .unwrap()
                .as_deref(),
            Some("Work/Editor")
        );
        for unsafe_path in ["../outside", "..\\outside", "/absolute", "C:\\outside"] {
            assert!(
                decode_legacy_folder(&format!("applications://~{}", encoded(unsafe_path))).is_err()
            );
        }
    }
}
