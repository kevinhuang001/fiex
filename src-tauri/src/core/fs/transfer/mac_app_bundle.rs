//! Native macOS application bundles import as executable units, never a
//! destructive move of an installed application. ditto preserves bundle links.
use super::{
    FsOperation, binding,
    planner::{collision, optional_stat, same_version, unique_application},
};
use crate::core::volumes::applications;
use crate::core::{
    operation_types::{Action, Resolution},
    operation_types::{OperationResult, check_cancelled},
};
use crate::plugin_sdk::filesystem::PreflightPhase;
use fiex_provider_contracts::TransferKind;
use std::path::Path;
use std::sync::atomic::AtomicBool;
#[allow(clippy::too_many_arguments)]
pub(super) fn import(
    paths: &[String],
    destination: &str,
    _moving: bool,
    keep_both: bool,
    resolutions: &[Resolution],
    cancel: &AtomicBool,
    progress: &impl Fn(u64, u64, Option<&str>),
) -> Result<Option<OperationResult>, String> {
    if paths.is_empty()
        || !(destination == applications::URI || applications::is_folder(destination))
    {
        return Ok(None);
    }
    let bundles = paths
        .iter()
        .filter(|p| {
            !p.contains("://")
                && Path::new(p)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("app"))
                && Path::new(p).is_dir()
        })
        .count();
    if bundles == 0 {
        return Ok(None);
    }
    if bundles != paths.len() {
        return Err(crate::i18n::t("请将应用包与其他文件分开导入").into());
    }
    let parent = binding::destination(destination, "core:application-import")?;
    let mut result = OperationResult::default();
    let mut planned = Vec::new();
    for path in paths {
        check_cancelled(cancel)?;
        let source = binding::trusted(path, "core:application-import")?;
        source
            .require(FsOperation::Read, false)
            .or_else(|_| source.require(FsOperation::List, false))?;
        let metadata = source.transfer_metadata()?;
        let name = metadata.name.clone();
        source.review_export(TransferKind::Copy, PreflightPhase::Execute)?;
        let mut target = binding::child(
            &parent,
            &name,
            crate::core::fs::provider_registry::ChildKind::Application,
        )?;
        let source_version = source.stat()?;
        if let Some(existing) = optional_stat(&target)? {
            match collision(
                &source,
                &target,
                &source_version,
                &existing,
                resolutions,
                keep_both,
                &mut result,
            ) {
                None => continue,
                Some(Action::Skip) => {
                    result.skipped += 1;
                    continue;
                }
                Some(Action::KeepBoth) => target = unique_application(&parent, &name)?,
                Some(Action::Replace) => {
                    return Err(crate::i18n::t("应用已存在，请重命名或选择保留两者").into());
                }
            }
        }
        target.require(FsOperation::Write, true)?;
        parent.require(FsOperation::Write, true)?;
        let final_name = target.path.rsplit(['/', '\\']).next().unwrap_or(&name);
        parent.review_import(
            TransferKind::Copy,
            final_name,
            &metadata,
            PreflightPhase::Execute,
        )?;
        planned.push((source, target, metadata, source_version));
    }
    if !result.conflicts.is_empty() {
        return Ok(Some(result));
    }
    for (index, (source, target, metadata, version)) in planned.iter().enumerate() {
        check_cancelled(cancel)?;
        let folder = applications::asset_path(&target.path)?;
        let staging_root = dirs::home_dir()
            .ok_or("cannot locate home directory")?
            .join(".fiex")
            .join("application-staging");
        std::fs::create_dir_all(&staging_root).map_err(|error| error.to_string())?;
        let staging = tempfile::Builder::new()
            .prefix(".app-import-")
            .tempdir_in(&staging_root)
            .map_err(|e| e.to_string())?;
        let staged = staging.path().join("bundle.app");
        let mut child = std::process::Command::new("/usr/bin/ditto")
            .arg(&source.path)
            .arg(&staged)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        loop {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                check_cancelled(cancel)?;
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err(crate::i18n::t("无法导入应用包").into());
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if !same_version(&source.stat()?, version) {
            return Err("Conflict: application changed during import".into());
        }
        let final_name = target
            .path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&metadata.name);
        parent.review_import(
            TransferKind::Copy,
            final_name,
            metadata,
            PreflightPhase::Execute,
        )?;
        target.require(FsOperation::Write, true)?;
        // Reserve an empty destination atomically. rename only replaces this
        // empty directory; newly added content makes the rename fail safely.
        std::fs::create_dir(&folder).map_err(|e| e.to_string())?;
        if let Err(error) = std::fs::rename(&staged, &folder) {
            let _ = std::fs::remove_dir_all(&folder);
            return Err(error.to_string());
        }
        applications::commit_reserved_application(&target.path)?;
        super::super::changes::committed(&target.path);
        result.paths.push(target.path.clone());
        progress(index as u64 + 1, planned.len() as u64, Some(&metadata.name));
    }
    Ok(Some(result))
}
