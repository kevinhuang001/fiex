use super::{binding, model::Step, FsOperation};
use crate::core::{
    fs::ResourceBroker,
    operations::{check_cancelled, OperationResult},
    transfers::{Action, Conflict, FileVersion, Resolution},
};
use crate::plugin_sdk::filesystem::PreflightPhase;
use fiex_provider_contracts::TransferKind;
use std::sync::atomic::AtomicBool;

pub(super) fn same_version(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    ["size", "modified", "isDir", "revision"]
        .iter()
        .all(|key| a[*key] == b[*key])
}

pub(super) fn is_missing(error: &str) -> bool {
    error.starts_with("NotFound:")
        || error.contains("No such file or directory")
        || error.contains("os error 2")
        || error.contains("not found")
}
pub(super) fn optional_stat(
    resource: &ResourceBroker,
) -> Result<Option<serde_json::Value>, String> {
    match resource.stat() {
        Ok(stat) => Ok(Some(stat)),
        Err(error) if is_missing(&error.to_string()) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn version(path: &str, stat: &serde_json::Value) -> FileVersion {
    FileVersion {
        path: path.into(),
        size: stat["size"].as_u64().unwrap_or(0),
        modified: stat["modified"].as_u64().unwrap_or(0),
        is_dir: stat["isDir"] == true,
    }
}

pub(super) fn collision(
    source: &ResourceBroker,
    target: &ResourceBroker,
    src: &serde_json::Value,
    dst: &serde_json::Value,
    choices: &[Resolution],
    keep_both: bool,
    result: &mut OperationResult,
) -> Option<Action> {
    let a = version(&source.path, src);
    let b = version(&target.path, dst);
    let key = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        a.path, b.path, a.modified, b.modified, a.size, b.size
    );
    if keep_both {
        return Some(Action::KeepBoth);
    }
    if let Some(choice) = choices.iter().find(|choice| choice.key == key) {
        return Some(choice.action);
    }
    result.conflicts.push(Conflict {
        key,
        source: a,
        destination: b,
    });
    None
}

pub(super) fn unique(
    parent: &ResourceBroker,
    name: &str,
    directory: bool,
) -> Result<ResourceBroker, String> {
    let path = std::path::Path::new(name);
    let stem = if directory {
        name
    } else {
        path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid file name")?
    };
    let extension = if directory {
        String::new()
    } else {
        path.extension()
            .and_then(|s| s.to_str())
            .map(|s| format!(".{s}"))
            .unwrap_or_default()
    };
    for index in 2..=10_000 {
        let target = binding::child(parent, &format!("{stem} ({index}){extension}"))?;
        if optional_stat(&target)?.is_none() {
            return Ok(target);
        }
    }
    Err("no free destination name".into())
}

pub(super) fn relative_target_name(
    policy_parent: &ResourceBroker,
    target: &ResourceBroker,
    fallback: &str,
) -> String {
    let prefix = format!("{}/", policy_parent.path.trim_end_matches(['/', '\\']));
    target
        .path
        .strip_prefix(&prefix)
        .filter(|relative| !relative.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn plan(
    source: ResourceBroker,
    parent: &ResourceBroker,
    name: &str,
    choices: &[Resolution],
    keep_both: bool,
    ignores: &[String],
    cancel: &AtomicBool,
    result: &mut OperationResult,
    steps: &mut Vec<Step>,
    depth: usize,
    action: TransferKind,
    policy_parent: &ResourceBroker,
) -> Result<(), String> {
    check_cancelled(cancel)?;
    if depth > 128 || steps.len() >= 100_000 {
        return Err("transfer tree limit exceeded".into());
    }
    if ignores
        .iter()
        .any(|ignore| ignore == name || ignore == &source.path)
    {
        result.skipped += 1;
        return Ok(());
    }
    let src = source.stat()?;
    if src["symlink"] == true {
        return Err("Unsupported: symlink transfer needs an explicit policy".into());
    }
    let directory = src["isDir"] == true;
    source.require(
        if directory {
            FsOperation::List
        } else {
            FsOperation::Read
        },
        false,
    )?;
    let mut target = binding::child(parent, name)?;
    if source.path == target.path
        || (directory
            && target
                .path
                .starts_with(&format!("{}/", source.path.trim_end_matches('/'))))
        || source
            .path
            .starts_with(&format!("{}/", target.path.trim_end_matches('/')))
    {
        return Err("destination is inside the source".into());
    }
    let mut existing = optional_stat(&target)?;
    if let Some(dst) = &existing {
        match collision(&source, &target, &src, dst, choices, keep_both, result) {
            None => return Ok(()),
            Some(Action::Skip) => {
                result.skipped += 1;
                return Ok(());
            }
            Some(Action::KeepBoth) => {
                target = unique(parent, name, directory)?;
                existing = None;
            }
            Some(Action::Replace) => {}
        }
    }
    // Replacing a file with a directory (or the inverse) would first
    // destroy the existing target. Reject the type change until the provider
    // can guarantee an atomic replacement with rollback.
    if existing
        .as_ref()
        .is_some_and(|entry| (entry["isDir"] == true) != directory)
    {
        return Err("Unsupported: replacement changes destination kind".into());
    }
    let metadata = source.transfer_metadata()?;
    let same_mount = source.reference.mount_id == parent.reference.mount_id;
    let final_name = target.path.rsplit(['/', '\\']).next().unwrap_or(name);
    let relative_name = relative_target_name(policy_parent, &target, final_name);
    if action == TransferKind::Move {
        source.review_remove(
            crate::plugin_sdk::filesystem::RemoveMode::Permanent,
            PreflightPhase::Execute,
        )?;
    }
    if same_mount && action == TransferKind::Move {
        source.review_move(policy_parent, Some(&relative_name), PreflightPhase::Execute)?;
    } else {
        source.review_export(action, PreflightPhase::Execute)?;
        policy_parent.require(
            if directory {
                FsOperation::Mkdir
            } else {
                FsOperation::Write
            },
            true,
        )?;
        policy_parent.review_import(action, &relative_name, &metadata, PreflightPhase::Execute)?;
    }
    let had_target = existing.is_some();
    let directory_source = source.clone_binding();
    steps.push(Step {
        source,
        target: target.clone_binding(),
        policy_parent: policy_parent.clone_binding(),
        action,
        directory,
        existing: had_target,
        source_version: src,
        target_version: existing,
    });
    if directory {
        let listing = directory_source.list()?;
        for entry in listing.as_array().ok_or("invalid directory listing")? {
            let child_name = entry["name"].as_str().ok_or("missing child name")?;
            let child_source = binding::child(&directory_source, child_name)?;
            // Descendant scopes are shared with their selected root. The
            // destination may not exist yet, but its ref is already confined.
            plan(
                child_source,
                &target,
                child_name,
                choices,
                false,
                ignores,
                cancel,
                result,
                steps,
                depth + 1,
                action,
                if had_target { &target } else { policy_parent },
            )?;
        }
    }
    Ok(())
}
