//! Executes a journaled Applications migration and finalizes recovery cleanup.
use super::files::{asset_filename, copy_node, fingerprint, remove_node_if_present};
use super::*;
use std::path::Path;

pub(super) fn execute_plan(mut plan: MigrationPlan, plan_file: &Path) -> Result<(), String> {
    let root = app_root()?;
    let stage_root = fiex_dir()?
        .join("application-staging")
        .join(format!("migration-{}", plan.migration_id));
    std::fs::create_dir_all(&stage_root).map_err(|error| error.to_string())?;
    for asset in &plan.assets {
        let source = root.join(safe_relative(&asset.source)?);
        let target = super::super::imported_asset_path(&asset.asset_id, &asset.extension)?;
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

pub(super) fn finish_migration(plan: &MigrationPlan, plan_file: &Path) -> Result<(), String> {
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
