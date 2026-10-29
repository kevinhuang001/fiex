//! One transfer planner for local, plugin and raw-device resources. Planning
//! finds conflicts before any writes; execution moves bytes in bounded chunks.
use super::{binding, FsOperation, ResourceBroker, CHUNK};
use crate::core::{
    operations::{check_cancelled, OperationResult},
    transfers::{Action, Conflict, FileVersion, Resolution},
};
use std::sync::atomic::AtomicBool;

mod rollback;
use rollback::UndoLog;
mod cleanup;
mod execution;

struct Step {
    source: ResourceBroker,
    target: ResourceBroker,
    directory: bool,
    existing: bool,
    source_version: serde_json::Value,
    target_version: Option<serde_json::Value>,
}

fn same_version(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    ["size", "modified", "isDir", "revision"]
        .iter()
        .all(|key| a[*key] == b[*key])
}

fn is_missing(error: &str) -> bool {
    error.starts_with("NotFound:")
        || error.contains("No such file or directory")
        || error.contains("os error 2")
        || error.contains("not found")
}
fn optional_stat(resource: &ResourceBroker) -> Result<Option<serde_json::Value>, String> {
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

fn collision(
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

fn unique(parent: &ResourceBroker, name: &str, directory: bool) -> Result<ResourceBroker, String> {
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

#[allow(clippy::too_many_arguments)]
fn plan(
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
    // destroy the existing target. Reject the type change until the adapter
    // can guarantee an atomic replacement with rollback.
    if existing
        .as_ref()
        .is_some_and(|entry| (entry["isDir"] == true) != directory)
    {
        return Err("Unsupported: replacement changes destination kind".into());
    }
    let had_target = existing.is_some();
    let directory_source = source.clone_binding();
    steps.push(Step {
        source,
        target: target.clone_binding(),
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
            )?;
        }
    }
    Ok(())
}

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
