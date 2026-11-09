//! One transfer planner for local, plugin and raw-device resources. Planning
//! finds conflicts before any writes; execution moves bytes in bounded chunks.
use super::{CHUNK, FsOperation, binding};
use crate::core::{
    operations::{OperationResult, check_cancelled},
    transfers::Resolution,
};
use crate::plugin_sdk::filesystem::PreflightPhase;
use fiex_provider_contracts::TransferKind;
use std::sync::atomic::AtomicBool;

mod cleanup;
mod execution;
#[cfg(target_os = "linux")]
mod linux_app_links;
#[cfg(target_os = "macos")]
mod mac_app_bundle;
mod model;
mod planner;
mod rollback;

use planner::plan;

#[allow(clippy::too_many_arguments)]
pub fn transfer(
    paths: &[String],
    destination: &str,
    moving: bool,
    keep_both: bool,
    ignores: &[String],
    resolutions: &[Resolution],
    cancel: &AtomicBool,
    progress: &impl Fn(u64, u64, Option<&str>),
) -> Result<OperationResult, String> {
    let mut effective_paths = paths.to_vec();
    let mut move_sources = Vec::new();
    let mut move_target = None;
    if moving && !paths.is_empty() {
        let target = binding::destination(destination, "core:transfer")?;
        let mut actionable = Vec::with_capacity(paths.len());
        for path in paths {
            let source = binding::trusted(path, "core:transfer")?;
            if source.path == target.path {
                continue;
            }
            let source_parent = binding::parent_path(&source.path)?;
            if source.reference.mount_id == target.reference.mount_id
                && source_parent == target.path
            {
                continue;
            }
            actionable.push(source.path.clone());
            move_sources.push(source);
        }
        if actionable.is_empty() {
            return Ok(OperationResult::default());
        }
        effective_paths = actionable;
        move_target = Some(target);
    }

    let applications = destination == crate::core::volumes::applications::URI
        || crate::core::volumes::applications::is_folder(destination);
    if !applications
        && effective_paths.iter().any(|path| {
            crate::core::volumes::applications::is_system_app(path)
                || crate::core::volumes::application_policy::protected(path)
        })
    {
        return Err(crate::i18n::t("系统应用不能移出应用程序视图").into());
    }
    if moving && !effective_paths.is_empty() {
        let target = move_target
            .as_ref()
            .ok_or("missing scoped move destination")?;
        if let Some(paths) =
            crate::core::volumes::native_vfs::move_within_mount(&move_sources, target)?
        {
            return Ok(OperationResult {
                paths,
                ..Default::default()
            });
        }
    }
    #[cfg(target_os = "macos")]
    if let Some(result) = mac_app_bundle::import(
        &effective_paths,
        destination,
        moving,
        keep_both,
        resolutions,
        cancel,
        progress,
    )? {
        return Ok(result);
    }
    #[cfg(target_os = "linux")]
    if let Some(result) = linux_app_links::import(&effective_paths, destination, keep_both, cancel)?
    {
        return Ok(result);
    }
    let parent = binding::destination(destination, "core:transfer")?;
    let mut result = OperationResult::default();
    let mut steps = Vec::new();
    let mut sources = Vec::new();
    for path in &effective_paths {
        check_cancelled(cancel)?;
        let source = binding::trusted(path, "core:transfer")?;
        if moving {
            source.require(FsOperation::Remove, false)?;
        }
        let name = source.stat()?["name"]
            .as_str()
            .ok_or("source has no name")?
            .to_owned();
        plan(
            source.clone_binding(),
            &parent,
            &name,
            resolutions,
            keep_both,
            ignores,
            cancel,
            &mut result,
            &mut steps,
            0,
            if moving {
                TransferKind::Move
            } else {
                TransferKind::Copy
            },
            &parent,
        )?;
        sources.push(source);
    }
    if !result.conflicts.is_empty() {
        return Ok(result);
    }
    if let Err(error) = execution::execute(&steps, cancel, progress) {
        result.errors.push(error);
        return Ok(result);
    }
    result.paths = steps
        .iter()
        .filter(|step| sources.iter().any(|source| source.path == step.source.path))
        .map(|step| step.target.path.clone())
        .collect();
    if moving && result.skipped == 0 && result.errors.is_empty() {
        if let Err(error) = cleanup::validate(&steps, cancel) {
            result.errors.push(error);
            return Ok(result);
        }
        // Delete leaves before directories. Providers can safely implement
        // remove as an empty-directory operation without a second tree engine.
        for step in steps.iter().rev() {
            if step.directory && !step.source.list()?.as_array().is_some_and(Vec::is_empty) {
                result.errors.push(format!(
                    "Conflict: source directory gained children: {}",
                    step.source.path
                ));
                break;
            }
            if let Err(error) = step.source.remove() {
                result.errors.push(format!("{}: {error}", step.source.path));
                break;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::transfer;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn moving_an_entry_into_its_current_parent_is_a_noop() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("still-here.txt");
        std::fs::write(&path, b"unchanged").unwrap();

        let result = transfer(
            &[path.to_string_lossy().into_owned()],
            &directory.path().to_string_lossy(),
            true,
            false,
            &[],
            &[],
            &AtomicBool::new(false),
            &|_, _, _| {},
        )
        .unwrap();

        assert!(result.paths.is_empty());
        assert!(result.errors.is_empty());
        assert!(result.conflicts.is_empty());
        assert_eq!(std::fs::read(path).unwrap(), b"unchanged");
    }
}
