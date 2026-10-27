//! Ref-only public file protocol. All host paths below this layer are Core
//! implementation details and must never be accepted from a plugin payload.
use super::{run, Actor, Running};
use crate::plugin_permission_policy::Capability;
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
        let parent = if let Some(snapshot) = scope.remote_target(&json!({"ref":reference}))? {
            if !snapshot.is_dir {
                return Err("parent ref must be a directory".into());
            }
            snapshot.display
        } else {
            let path = scope.resolve_reference(reference)?;
            if !crate::plugin_fs_access::is_directory(scope, &path)? {
                return Err("parent ref must be a directory".into());
            }
            path
        };
        let child = if crate::mounts::handles(&parent) {
            let uri = crate::mounts::parse(&parent)?;
            crate::mounts::uri(
                &uri.mount_id,
                format!("{}/{}", uri.relative.trim_end_matches('/'), name).trim_start_matches('/'),
            )?
        } else {
            std::path::Path::new(&parent)
                .join(name)
                .to_string_lossy()
                .into_owned()
        };
        return scope.issue_child(reference, &child, None).map(Some);
    }
    const TARGET_METHODS: &[&str] = &[
        "host.fs.openRead",
        "host.fs.readChunk",
        "host.fs.list",
        "host.fs.stat",
        "host.fs.beginWrite",
        "host.fs.mkdir",
        "host.fs.remove",
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
    if method == "host.fs.stat" {
        running.ensure_permission(Capability::Metadata)?;
    }
    Ok(None)
}

pub(super) fn metadata(value: &Value) -> Value {
    json!({"name":value["name"],"kind":if value["isDir"]==true || value["kind"]=="directory" {"directory"}else if value["symlink"]==true {"symlink"}else{"file"},
        "extension":value["extension"],"mime":value["mime"],"size":value["size"],"modified":value["modified"]})
}
pub(super) fn listing(
    scope: &crate::plugin_fs_access::Scope,
    parent: &Value,
    values: Value,
) -> Result<Value, String> {
    let values = values.as_array().ok_or("invalid directory listing")?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let raw = value["path"]
            .as_str()
            .ok_or("entry missing internal path")?;
        let display = if crate::mounts::handles(raw) || std::path::Path::new(raw).is_absolute() {
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
