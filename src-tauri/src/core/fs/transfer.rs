//! One transfer planner for local, plugin and raw-device resources. Planning
//! finds conflicts before any writes; execution moves bytes in bounded chunks.
use super::{binding, FsOperation, CHUNK};
use crate::core::{
    operations::{check_cancelled, OperationResult},
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
    let applications = destination == crate::core::volumes::applications::URI
        || crate::core::volumes::applications::is_user(destination);
    if !applications
        && paths.iter().any(|path| {
            crate::core::volumes::applications::is_system_app(path)
                || crate::core::volumes::application_policy::protected(path)
        })
    {
        return Err(crate::i18n::t("系统应用不能移出应用程序视图").into());
    }
    #[cfg(target_os = "macos")]
    if let Some(result) = mac_app_bundle::import(
        paths,
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
    if let Some(result) = linux_app_links::import(paths, destination, keep_both, cancel)? {
        return Ok(result);
    }
    // Installed applications are not host files. Moving them into/out of an
    // Applications folder only rewrites the persisted mapping; user-store
    // entries in the same selection still go through the regular engine.
    if moving {
        let system: Vec<String> = paths
            .iter()
            .filter(|path| {
                crate::core::volumes::applications::is_system_app(path)
                    && crate::core::volumes::applications::system_entry(path).is_ok()
            })
            .cloned()
            .collect();
        if !system.is_empty() {
            if destination != crate::core::volumes::applications::URI
                && !crate::core::volumes::applications::is_user(destination)
            {
                return Err(
                    "system applications can only be moved inside the Applications view".into(),
                );
            }
            let rest: Vec<String> = paths
                .iter()
                .filter(|path| !system.iter().any(|candidate| candidate == *path))
                .cloned()
                .collect();
            let parent = binding::destination(destination, "core:transfer")?;
            for path in &system {
                let source = binding::trusted(path, "core:transfer")?;
                let metadata = source.transfer_metadata()?;
                source.review_move(&parent, Some(&metadata.name), PreflightPhase::Execute)?;
            }
            crate::core::volumes::applications::move_system_apps(&system, destination)?;
            let mut result = OperationResult {
                paths: system,
                ..Default::default()
            };
            if rest.is_empty() {
                return Ok(result);
            }
            let mut other = transfer(
                &rest,
                destination,
                moving,
                keep_both,
                ignores,
                resolutions,
                cancel,
                progress,
            )?;
            result.paths.append(&mut other.paths);
            result.errors.append(&mut other.errors);
            result.skipped += other.skipped;
            result.conflicts.append(&mut other.conflicts);
            return Ok(result);
        }
    }
    let parent = binding::destination(destination, "core:transfer")?;
    let mut result = OperationResult::default();
    let mut steps = Vec::new();
    let mut sources = Vec::new();
    for path in paths {
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
