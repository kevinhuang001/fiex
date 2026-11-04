//! Ref-only public file protocol. All host paths below this layer are Core
//! implementation details and must never be accepted from a plugin payload.
use super::{run, Actor, Running};
use crate::plugins::{provider_registry::FsOperation, security::policy::Capability};
use serde_json::{json, Value};

pub(super) fn intercept(
    running: &Running,
    actor: &Actor,
    method: &str,
    params: &Value,
) -> Result<Option<Value>, String> {
    if !method.starts_with("host.fs.") {
        return Ok(None);
    }
    for key in ["path", "paths", "from", "to"] {
        if params.get(key).is_some() {
            return Err(
                "filesystem methods require ResourceRef; host paths are not accepted".into(),
            );
        }
    }
    if method == "host.fs.statBatch" {
        let refs = params["refs"]
            .as_array()
            .filter(|refs| refs.len() <= 256)
            .ok_or("invalid refs batch")?;
        let mut results = Vec::with_capacity(refs.len());
        for reference in refs {
            let mut item = params.clone();
            item["ref"] = reference.clone();
            results.push(match run(running, "host.fs.stat", &item) {
                Ok(metadata) => json!({"metadata":metadata}),
                Err(error) => json!({"error":error}),
            });
        }
        return Ok(Some(json!({"results":results})));
    }
    if method == "host.fs.resolveMapped" {
        let scope = actor.scope.as_ref().ok_or("no operation scope")?;
        let locator = params
            .get("locator")
            .and_then(Value::as_str)
            .filter(|locator| !locator.is_empty() && locator.len() <= 4096)
            .ok_or("invalid mapped resource locator")?;
        let operation = params
            .get("operation")
            .and_then(Value::as_str)
            .and_then(FsOperation::parse)
            .ok_or("invalid mapped resource operation")?;
        if operation == FsOperation::Search {
            return Err("Unsupported: mapped search is unavailable through Host.fs".into());
        }
        let user_action = scope.user_action_context();
        let bound = scope
            .reference_for_path(locator)
            .ok()
            .and_then(|reference| scope.resource_bound(&reference).ok())
            .unwrap_or(false);
        if !bound && user_action.is_none() {
            return Err("NeedsAuthorization: mapped resource requires a user action".into());
        }
        let snapshot =
            crate::plugins::resource::resource_context::ResourceSnapshot::resolve(locator)?;
        running.ensure_permission(mapped_capability(operation, snapshot.is_dir))?;
        let (resource, snapshot) = crate::core::fs::mapped_access::authorize_snapshot(
            snapshot,
            scope,
            &actor.owner,
            user_action,
            operation,
        )?;
        scope.bind_mapped_resource(snapshot, operation)?;
        return Ok(Some(json!({"ref":resource.reference})));
    }
    if method == "host.fs.child" {
        let scope = actor.scope.as_ref().ok_or("no operation scope")?;
        let reference = params.get("ref").ok_or("missing parent ref")?;
        let name = params["name"]
            .as_str()
            .filter(|name| {
                !name.is_empty()
                    && !matches!(*name, "." | "..")
                    && !name.contains(['/', '\\', ':', '\0'])
            })
            .ok_or("invalid child name")?;
        let resource = crate::core::fs::ResourceBroker::resolve(scope, &actor.owner, reference)?;
        if !resource.is_dir()? {
            return Err("parent ref must be a directory".into());
        }
        let child = crate::core::fs::child(
            &resource,
            name,
            crate::core::fs::provider_registry::ChildKind::File,
        )?;
        return Ok(Some(json!({"ref":child.reference})));
    }
    const TARGET_METHODS: &[&str] = &[
        "host.fs.openRead",
        "host.fs.readChunk",
        "host.fs.list",
        "host.fs.stat",
        "host.fs.beginWrite",
        "host.fs.mkdir",
        "host.fs.remove",
        "host.fs.trash",
        "host.fs.rename",
    ];
    if TARGET_METHODS.contains(&method) && params.get("ref").is_none() {
        return Err("missing ResourceRef".into());
    }
    // Scope first; an unbound ref never causes a permission prompt.
    if let Some(reference) = params.get("ref") {
        if !actor
            .scope
            .as_ref()
            .ok_or("no operation scope")?
            .resource_bound(reference)?
        {
            return Err("resource not authorized in this operation".into());
        }
    }
    Ok(None)
}

fn mapped_capability(operation: FsOperation, is_dir: bool) -> Capability {
    match operation {
        FsOperation::List | FsOperation::Search => Capability::FoldersRead,
        FsOperation::Stat => Capability::Metadata,
        FsOperation::Read => Capability::FilesRead,
        FsOperation::Write => Capability::FilesWrite,
        FsOperation::Mkdir => Capability::FoldersCreate,
        FsOperation::Rename if is_dir => Capability::FoldersRename,
        FsOperation::Rename => Capability::FilesRename,
        FsOperation::Trash | FsOperation::Remove if is_dir => Capability::FoldersDelete,
        FsOperation::Trash | FsOperation::Remove => Capability::FilesDelete,
    }
}

pub(super) fn metadata(value: &Value) -> Value {
    json!({"name":value["name"],"kind":if value["symlink"]==true || value["isSymlink"]==true || value["kind"]=="symlink" {"symlink"}else if value["isDir"]==true || value["kind"]=="directory" {"directory"}else{"file"},
        "extension":value["extension"],"mime":value["mime"],"size":value["size"],"modified":value["modified"]})
}
pub(super) fn listing(
    scope: &crate::plugins::resource::fs_access::Scope,
    parent: &Value,
    values: Value,
) -> Result<Value, String> {
    let values = values.as_array().ok_or("invalid directory listing")?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let raw = value["path"]
            .as_str()
            .ok_or("entry missing internal path")?;
        let display =
            if crate::plugins::mounts::handles(raw) || std::path::Path::new(raw).is_absolute() {
                raw.to_string()
            } else {
                scope.path().join(raw).to_string_lossy().into_owned()
            };
        let metadata =
            serde_json::from_value(metadata(value)).map_err(|error| error.to_string())?;
        result.push(scope.issue_child(parent, &display, Some(metadata))?);
    }
    Ok(json!(result))
}
