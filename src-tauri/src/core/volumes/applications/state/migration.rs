//! One-time, recoverable conversion of the old physical Applications tree.
//! After completion the old URI and JSON mapping are no longer read by normal
//! Applications operations.
mod execution;
mod files;
mod plan;
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
};

const PLAN_FILE: &str = "applications-migration-v1.json";

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
    fiex_applications::applications_root_path()
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
                execution::finish_migration(&plan, &plan_file)?;
            } else {
                return Err("Applications migration journal does not match stored state".into());
            }
        }
        return Ok(());
    }
    if plan_file.is_file() {
        let plan = load_plan(&plan_file)?;
        execution::execute_plan(plan, &plan_file)?;
        return Ok(());
    }
    if !plan::legacy_plan_exists()? {
        return Ok(());
    }
    let plan = plan::build_plan()?;
    save_plan(&plan_file, &plan)?;
    execution::execute_plan(plan, &plan_file)
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

#[cfg(test)]
mod tests {
    #[test]
    fn safe_relative_rejects_escape_paths() {
        for path in ["../outside", "..\\outside", "/absolute", "C:\\outside"] {
            assert!(super::safe_relative(path).is_err());
        }
    }
}
