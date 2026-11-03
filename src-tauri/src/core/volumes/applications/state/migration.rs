//! One-time, recoverable conversion of the old physical Applications tree.
//! After completion the old URI and JSON mapping are no longer read by normal
//! Applications operations.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
};

const PLAN_FILE: &str = "applications-migration-v1.json";

#[derive(Default, Deserialize)]
struct LegacyFolderStore {
    #[serde(default)]
    folders: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AssetMove {
    source: String,
    asset_id: String,
    extension: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MigrationPlan {
    version: u32,
    migration_id: String,
    committed: bool,
    state: ApplicationsState,
    assets: Vec<AssetMove>,
    recovery: Vec<String>,
    directories: Vec<String>,
    has_legacy_folder_map: bool,
    warnings: Vec<String>,
}

fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "cannot locate home directory".into())
}

fn fiex_dir() -> Result<PathBuf, String> {
    Ok(home()?.join(".fiex"))
}

fn app_root() -> Result<PathBuf, String> {
    super::applications_root_path()
}

fn plan_path() -> Result<PathBuf, String> {
    Ok(fiex_dir()?.join("application-staging").join(PLAN_FILE))
}

fn recovery_root(migration_id: &str) -> Result<PathBuf, String> {
    Ok(fiex_dir()?.join("application-recovery").join(migration_id))
}

fn legacy_folder_path() -> Result<PathBuf, String> {
    Ok(fiex_dir()?.join("application-folders.json"))
}

fn migration_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn safe_relative(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains(['\\', ':', '\0'])
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("invalid legacy Applications relative path".into());
    }
    Ok(path.to_path_buf())
}

fn decode_legacy_folder(uri: &str) -> Result<Option<String>, String> {
    if uri == super::super::URI {
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
    safe_relative(&normalized)?;
    Ok(Some(normalized))
}

fn legacy_plan_exists() -> Result<bool, String> {
    let root = app_root()?;
    let has_tree = std::fs::read_dir(&root)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some();
    Ok(has_tree || legacy_folder_path()?.is_file())
}

pub(super) fn ensure_migrated() -> Result<(), String> {
    let _guard = migration_lock()
        .lock()
        .map_err(|_| "Applications migration lock poisoned")?;
    let state_storage = storage();
    let plan_file = plan_path()?;
    let current = state_storage.get(KEY)?;
    if let Some(current) = current {
        if plan_file.is_file() {
            let mut plan = load_plan(&plan_file)?;
            let state: ApplicationsState =
                serde_json::from_value(current).map_err(|error| error.to_string())?;
            if state.migration_id.as_deref() == Some(&plan.migration_id) {
                plan.committed = true;
                save_plan(&plan_file, &plan)?;
                finish_migration(&plan, &plan_file)?;
            } else {
                return Err("Applications migration journal does not match stored state".into());
            }
        }
        return Ok(());
    }
    if plan_file.is_file() {
        let plan = load_plan(&plan_file)?;
        execute_plan(plan, &plan_file)?;
        return Ok(());
    }
    if !legacy_plan_exists()? {
        return Ok(());
    }
    let plan = build_plan()?;
    save_plan(&plan_file, &plan)?;
    execute_plan(plan, &plan_file)
}

fn build_plan() -> Result<MigrationPlan, String> {
    let root = app_root()?;
    let migration_id = super::random_id()?;
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
        scan_directory(
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
        let system_entries = super::super::platform::platform_list().unwrap_or_default();
        let mut by_old_uri = HashMap::new();
        for entry in system_entries {
            let encoded = entry
                .path
                .strip_prefix(super::super::URI)
                .ok_or("system application has invalid legacy URI")?;
            let locator = super::super::decode(encoded)?;
            by_old_uri.insert(entry.path, (locator, entry.name));
        }
        let mut assigned = HashSet::new();
        for (folder_uri, old_apps) in map.folders {
            let relative = decode_legacy_folder(&folder_uri)?;
            let parent_id = relative
                .as_deref()
                .map(|path| ensure_folder_path(path, &mut state, &mut folder_by_relative))
                .transpose()?;
            for old_app in old_apps {
                let (locator, display_name, unavailable) = if let Some((locator, display_name)) =
                    by_old_uri.get(&old_app)
                {
                    (locator.clone(), display_name.clone(), false)
                } else if let Some(encoded) = old_app.strip_prefix(super::super::URI) {
                    match super::super::decode(encoded) {
                        Ok(locator) if !locator.is_empty() => {
                            let fallback = fallback_system_name(&locator);
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
                let id = super::system_id(&locator);
                if !assigned.insert(id.clone()) {
                    warnings.push(format!("duplicate system application mapping: {old_app}"));
                    continue;
                }
                let order = super::next_order(&state, parent_id.as_deref());
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

fn fallback_system_name(locator: &str) -> String {
    let path = locator
        .strip_prefix("path:")
        .or_else(|| locator.strip_prefix("aumid:"))
        .unwrap_or(locator)
        .replace('\\', "/");
    let candidate = if let Some((_, app_id)) = path.rsplit_once('!') {
        app_id
    } else {
        Path::new(&path)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or(&path)
    };
    let safe = candidate
        .chars()
        .filter(|character| !matches!(character, '/' | '\\' | ':' | '\0'))
        .collect::<String>()
        .trim()
        .to_owned();
    if safe.is_empty() {
        "Unavailable application".into()
    } else {
        safe
    }
}

#[allow(clippy::too_many_arguments)]
fn scan_directory(
    root: &Path,
    relative_dir: &Path,
    parent_id: Option<String>,
    state: &mut ApplicationsState,
    folder_by_relative: &mut HashMap<String, Option<String>>,
    assets: &mut Vec<AssetMove>,
    recovery: &mut Vec<String>,
    directories: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let directory = root.join(relative_dir);
    let mut children = std::fs::read_dir(&directory)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        let relative = relative_dir.join(child.file_name());
        let relative_text = relative
            .to_str()
            .ok_or("legacy Applications contains a non-UTF8 path")?
            .replace('\\', "/");
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            recovery.push(relative_text);
            continue;
        }
        if metadata.is_dir()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
            && !is_application(&path, true)
        {
            recovery.push(relative_text);
        } else if metadata.is_dir() && !is_application(&path, true) {
            let id = ensure_folder_path(&relative_text, state, folder_by_relative)?;
            directories.push(relative_text);
            scan_directory(
                root,
                &relative,
                Some(id),
                state,
                folder_by_relative,
                assets,
                recovery,
                directories,
                warnings,
            )?;
        } else if is_application(&path, metadata.is_dir()) {
            let id = super::random_id()?;
            let asset_id = super::random_id()?;
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or("legacy application has an invalid name")?
                .to_owned();
            let order = super::next_order(state, parent_id.as_deref());
            state.apps.insert(
                id.clone(),
                ApplicationNode {
                    id,
                    parent: parent_id.clone(),
                    display_name: name,
                    source: ApplicationSource::Imported {
                        asset_id: asset_id.clone(),
                        extension: extension.clone(),
                    },
                    order,
                    unavailable: false,
                },
            );
            assets.push(AssetMove {
                source: relative_text,
                asset_id,
                extension,
            });
        } else if metadata.is_file() {
            recovery.push(relative_text);
        } else {
            warnings.push(format!("unsupported legacy node retained: {relative_text}"));
        }
    }
    Ok(())
}

fn is_application(path: &Path, is_dir: bool) -> bool {
    if is_dir
        && !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
    {
        return false;
    }
    let target = crate::core::files::entry(path)
        .ok()
        .and_then(|entry| entry.link_target);
    super::super::classify_host_application(&path.to_string_lossy(), target.as_deref())
}

fn ensure_folder_path(
    relative: &str,
    state: &mut ApplicationsState,
    folder_by_relative: &mut HashMap<String, Option<String>>,
) -> Result<String, String> {
    let path = safe_relative(relative)?;
    let mut parent_rel = String::new();
    let mut parent_id = None;
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err("invalid Applications folder component".into());
        };
        let name = component
            .to_str()
            .ok_or("legacy Applications contains a non-UTF8 folder name")?;
        parent_rel = if parent_rel.is_empty() {
            name.to_owned()
        } else {
            format!("{parent_rel}/{name}")
        };
        if let Some(existing) = folder_by_relative.get(&parent_rel) {
            parent_id = existing.clone();
            continue;
        }
        let id = super::random_id()?;
        let order = super::next_order(state, parent_id.as_deref());
        state.folders.insert(
            id.clone(),
            VirtualFolder {
                id: id.clone(),
                parent: parent_id.clone(),
                name: name.to_owned(),
                order,
            },
        );
        folder_by_relative.insert(parent_rel.clone(), Some(id.clone()));
        parent_id = Some(id);
    }
    parent_id.ok_or_else(|| "legacy Applications folder has no components".into())
}

fn load_plan(path: &Path) -> Result<MigrationPlan, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let plan: MigrationPlan = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if plan.version != 1 || !super::valid_id(&plan.migration_id) {
        return Err("unsupported Applications migration journal version".into());
    }
    if plan.state.migration_id.as_deref() != Some(&plan.migration_id) {
        return Err("Applications migration journal identity mismatch".into());
    }
    super::validate_state(&plan.state)?;
    for asset in &plan.assets {
        safe_relative(&asset.source)?;
        if !super::valid_id(&asset.asset_id) || !super::valid_extension(&asset.extension) {
            return Err("invalid application asset in migration journal".into());
        }
    }
    for relative in plan.recovery.iter().chain(plan.directories.iter()) {
        safe_relative(relative)?;
    }
    Ok(plan)
}

fn save_plan(path: &Path, plan: &MigrationPlan) -> Result<(), String> {
    let parent = path.parent().ok_or("migration journal parent missing")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(plan).map_err(|error| error.to_string())?;
    let temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    std::fs::write(temporary.path(), bytes).map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

fn execute_plan(mut plan: MigrationPlan, plan_file: &Path) -> Result<(), String> {
    let root = app_root()?;
    let stage_root = fiex_dir()?
        .join("application-staging")
        .join(format!("migration-{}", plan.migration_id));
    std::fs::create_dir_all(&stage_root).map_err(|error| error.to_string())?;
    for asset in &plan.assets {
        let source = root.join(safe_relative(&asset.source)?);
        let target = super::imported_asset_path(&asset.asset_id, &asset.extension)?;
        if target.exists() {
            if !source.exists() || fingerprint(&source)? != fingerprint(&target)? {
                return Err(format!(
                    "Applications migration target does not match its source: {}",
                    asset.source
                ));
            }
            continue;
        }
        let stage = stage_root.join(asset_filename(&asset.asset_id, &asset.extension));
        if stage.exists() && (!source.exists() || fingerprint(&source)? != fingerprint(&stage)?) {
            remove_node_if_present(&stage)?;
        }
        if !stage.exists() {
            copy_node(&source, &stage)?;
        }
        if !source.exists() || fingerprint(&source)? != fingerprint(&stage)? {
            return Err(format!(
                "Applications migration verification failed: {}",
                asset.source
            ));
        }
        std::fs::rename(&stage, &target).map_err(|error| error.to_string())?;
    }
    let recovery_root = recovery_root(&plan.migration_id)?;
    for relative in &plan.recovery {
        let source = root.join(safe_relative(relative)?);
        let target = recovery_root.join(safe_relative(relative)?);
        if target.exists() {
            if source.exists() {
                if fingerprint(&source)? == fingerprint(&target)? {
                    continue;
                }
                remove_node_if_present(&target)?;
            } else {
                return Err(format!(
                    "Applications recovery source is missing: {relative}"
                ));
            }
        }
        if !source.exists() {
            return Err(format!(
                "Applications recovery source is missing: {relative}"
            ));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        copy_node(&source, &target)?;
        if fingerprint(&source)? != fingerprint(&target)? {
            return Err(format!(
                "Applications recovery verification failed: {relative}"
            ));
        }
    }
    if plan.has_legacy_folder_map {
        let source = legacy_folder_path()?;
        let backup = recovery_root.join("application-folders.json");
        if source.is_file() && !backup.exists() {
            std::fs::create_dir_all(&recovery_root).map_err(|error| error.to_string())?;
            std::fs::copy(source, backup).map_err(|error| error.to_string())?;
        }
    }
    write_report(&plan, &recovery_root)?;
    let value = serde_json::to_value(&plan.state).map_err(|error| error.to_string())?;
    let state_storage = storage();
    if !state_storage.compare_and_set(KEY, None, Some(value))? {
        let Some(existing) = state_storage.get(KEY)? else {
            return Err("Applications migration lost its atomic state commit".into());
        };
        let existing: ApplicationsState =
            serde_json::from_value(existing).map_err(|error| error.to_string())?;
        if existing.migration_id.as_deref() != Some(&plan.migration_id) {
            return Err("Applications state changed during one-time migration".into());
        }
    }
    plan.committed = true;
    save_plan(plan_file, &plan)?;
    finish_migration(&plan, plan_file)
}

fn asset_filename(asset_id: &str, extension: &str) -> String {
    if extension.is_empty() {
        asset_id.to_owned()
    } else {
        format!("{asset_id}.{extension}")
    }
}

fn copy_node(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if metadata.file_type().is_symlink() {
        let link = std::fs::read_link(source).map_err(|error| error.to_string())?;
        create_symlink(&link, target, metadata.is_dir())?;
        return Ok(());
    }
    if metadata.is_dir() {
        std::fs::create_dir(target).map_err(|error| error.to_string())?;
        let mut children = std::fs::read_dir(source)
            .map_err(|error| error.to_string())?
            .map(|entry| entry.map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            copy_node(&child.path(), &target.join(child.file_name()))?;
        }
        std::fs::set_permissions(target, metadata.permissions())
            .map_err(|error| error.to_string())?;
    } else if metadata.is_file() {
        std::fs::copy(source, target).map_err(|error| error.to_string())?;
        std::fs::set_permissions(target, metadata.permissions())
            .map_err(|error| error.to_string())?;
    } else {
        return Err(format!("unsupported migration node: {}", source.display()));
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path, directory: bool) -> Result<(), String> {
    let _ = directory;
    std::os::unix::fs::symlink(target, link).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path, directory: bool) -> Result<(), String> {
    if directory {
        std::os::windows::fs::symlink_dir(target, link).map_err(|error| error.to_string())
    } else {
        std::os::windows::fs::symlink_file(target, link).map_err(|error| error.to_string())
    }
}

#[cfg(not(any(unix, windows)))]
fn create_symlink(_target: &Path, _link: &Path, _directory: bool) -> Result<(), String> {
    Err("symbolic links cannot be migrated on this platform".into())
}

fn fingerprint(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    fn walk(root: &Path, current: &Path, digest: &mut Sha256) -> Result<(), String> {
        let metadata = std::fs::symlink_metadata(current).map_err(|error| error.to_string())?;
        let rel = current
            .strip_prefix(root)
            .unwrap_or(current)
            .to_string_lossy();
        digest.update(rel.as_bytes());
        if metadata.file_type().is_symlink() {
            digest.update(*b"l");
            digest.update(
                std::fs::read_link(current)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .as_bytes(),
            );
        } else if metadata.is_dir() {
            digest.update(*b"d");
            let mut children = std::fs::read_dir(current)
                .map_err(|error| error.to_string())?
                .map(|entry| entry.map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            children.sort_by_key(|entry| entry.file_name());
            for child in children {
                walk(root, &child.path(), digest)?;
            }
        } else if metadata.is_file() {
            digest.update(*b"f");
            digest.update(metadata.len().to_le_bytes());
            let mut file = std::fs::File::open(current).map_err(|error| error.to_string())?;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                use std::io::Read;
                let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
                if read == 0 {
                    break;
                }
                digest.update(&buffer[..read]);
            }
        }
        Ok(())
    }
    let mut digest = Sha256::new();
    walk(path, path, &mut digest)?;
    Ok(hex::encode(digest.finalize()))
}

fn write_report(plan: &MigrationPlan, recovery_root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(recovery_root).map_err(|error| error.to_string())?;
    let mut report = String::from(
        "# Applications migration recovery\n\nLegacy non-application files were moved here so they are not exposed as Applications entries.\n\n",
    );
    if plan.recovery.is_empty() {
        report.push_str("No non-application files required recovery.\n");
    } else {
        report.push_str("Recovered items:\n");
        for item in &plan.recovery {
            report.push_str(&format!("- `{item}`\n"));
        }
    }
    if !plan.warnings.is_empty() {
        report.push_str("\nWarnings:\n");
        for warning in &plan.warnings {
            report.push_str(&format!("- {warning}\n"));
        }
    }
    std::fs::write(recovery_root.join("REPORT.md"), report).map_err(|error| error.to_string())
}

fn finish_migration(plan: &MigrationPlan, plan_file: &Path) -> Result<(), String> {
    let root = app_root()?;
    for asset in &plan.assets {
        let source = root.join(safe_relative(&asset.source)?);
        remove_node_if_present(&source)?;
    }
    for relative in &plan.recovery {
        let source = root.join(safe_relative(relative)?);
        remove_node_if_present(&source)?;
    }
    let mut directories = plan.directories.clone();
    directories.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for relative in directories {
        let directory = root.join(safe_relative(&relative)?);
        match std::fs::remove_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
            Err(error) if error.raw_os_error() == Some(145) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    if plan.has_legacy_folder_map {
        let legacy = legacy_folder_path()?;
        if legacy.exists() {
            std::fs::remove_file(legacy).map_err(|error| error.to_string())?;
        }
    }
    let stage = plan_file.parent().ok_or("migration staging root missing")?;
    let migration_stage = stage.join(format!("migration-{}", plan.migration_id));
    if migration_stage.exists() {
        std::fs::remove_dir_all(migration_stage).map_err(|error| error.to_string())?;
    }
    if plan_file.exists() {
        std::fs::remove_file(plan_file).map_err(|error| error.to_string())?;
    }
    if !plan.warnings.is_empty() {
        crate::infrastructure::logging::record_core(
            "warn",
            "applications.migration",
            &plan.warnings.join("; "),
        );
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
            decode_legacy_folder(super::super::super::URI).unwrap(),
            None
        );
        assert_eq!(
            decode_legacy_folder(&format!(
                "{}~{}",
                super::super::super::URI,
                encoded("Work/Editor")
            ))
            .unwrap()
            .as_deref(),
            Some("Work/Editor")
        );
        for unsafe_path in ["../outside", "..\\outside", "/absolute", "C:\\outside"] {
            assert!(decode_legacy_folder(&format!(
                "{}~{}",
                super::super::super::URI,
                encoded(unsafe_path)
            ))
            .is_err());
        }
    }

    #[test]
    fn unavailable_windows_apps_keep_a_displayable_name() {
        assert_eq!(
            fallback_system_name("aumid:Vendor.Package!Editor"),
            "Editor"
        );
        assert_eq!(fallback_system_name("path:C:\\Apps\\Editor.exe"), "Editor");
    }

    #[test]
    fn migration_copy_verification_covers_nested_assets_and_symlink_targets() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let app = source.path().join("Example.app");
        std::fs::create_dir(&app).unwrap();
        std::fs::write(app.join("binary"), b"application bytes").unwrap();
        let copied = destination.path().join("Example.app");
        copy_node(&app, &copied).unwrap();
        assert_eq!(fingerprint(&app).unwrap(), fingerprint(&copied).unwrap());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = source.path().join("link");
            symlink("missing-target", &link).unwrap();
            let copied_link = destination.path().join("link");
            copy_node(&link, &copied_link).unwrap();
            assert_eq!(
                fingerprint(&link).unwrap(),
                fingerprint(&copied_link).unwrap()
            );
        }
    }
}
