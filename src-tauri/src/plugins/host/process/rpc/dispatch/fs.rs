//! Ref-only Feature → Core service. Scope is validated before any permission
//! dialog; every storage operation then goes through the ResourceBroker.
use super::{refs, Context};
use crate::{
    core::fs::{self, ResourceBroker},
    plugins::{provider_registry::FsOperation, security::policy::Capability},
};
use serde_json::{json, Value};

pub(super) fn dispatch(ctx: &mut Context<'_>) -> Result<Value, String> {
    let owner = ctx.owner();
    if matches!(
        ctx.method,
        "fs.readOpen"
            | "fs.closeRead"
            | "fs.seekWrite"
            | "fs.writeChunk"
            | "fs.writeBuffer"
            | "fs.commit"
            | "fs.abort"
    ) {
        // Look up the actor-bound handle before inspecting grants. Guessing a
        // handle owned by another task can never cause an authorization prompt.
        let permission = fs::stream_permission(&owner, &ctx.arg("handle")?)?;
        if !matches!(ctx.method, "fs.closeRead" | "fs.abort") {
            ctx.running.require_permission(
                Capability::from_name(permission).ok_or("invalid stream permission")?,
            )?;
        }
        return fs::stream_call(&owner, ctx.method, ctx.params);
    }
    let scope = ctx.scope()?;
    let reference = ctx.params.get("ref").ok_or("missing ResourceRef")?;
    // Metadata-only discovery may project a captured target without exposing
    // a local path or opening a storage stream.
    if ctx.method == "fs.stat"
        && scope.resolve_reference(reference).is_err()
        && scope.remote_target(&json!({"ref":reference}))?.is_none()
    {
        if let Some(metadata) = scope.metadata_reference(reference)? {
            ctx.running.ensure_permission(Capability::Metadata)?;
            return Ok(refs::metadata(metadata));
        }
    }
    let resource = ResourceBroker::resolve(&scope, &owner, reference)?;
    let create = ctx.params["create"].as_bool().unwrap_or(false);
    let (op, permission) = match ctx.method {
        "fs.stat" => (FsOperation::Stat, Capability::Metadata),
        "fs.list" => (FsOperation::List, Capability::FoldersRead),
        "fs.openRead" | "fs.readChunk" => (FsOperation::Read, Capability::FilesRead),
        "fs.beginWrite" => (
            FsOperation::Write,
            if create {
                Capability::FilesCreate
            } else {
                Capability::FilesWrite
            },
        ),
        "fs.mkdir" => (FsOperation::Mkdir, Capability::FoldersCreate),
        "fs.remove" => (
            FsOperation::Remove,
            if resource.is_dir()? {
                Capability::FoldersDelete
            } else {
                Capability::FilesDelete
            },
        ),
        "fs.rename" => (
            FsOperation::Rename,
            if resource.is_dir()? {
                Capability::FoldersRename
            } else {
                Capability::FilesRename
            },
        ),
        _ => return Err("unsupported filesystem method".into()),
    };
    // Resolve the destination before asking for source permission too.
    let destination = if ctx.method == "fs.rename" {
        Some(ResourceBroker::resolve(
            &scope,
            &owner,
            ctx.params
                .get("destination")
                .ok_or("missing destination ref")?,
        )?)
    } else {
        None
    };
    ctx.running.ensure_permission(permission)?;
    resource.require(op, create)?;
    if let Some(destination) = &destination {
        if resource.reference.mount_id != destination.reference.mount_id {
            return Err("Unsupported: cross-mount rename".into());
        }
        // Parent creation rights are independent of the source's rename right.
        destination.require(FsOperation::Write, true)?;
    }
    match ctx.method {
        "fs.stat" => Ok(refs::metadata(&resource.stat()?)),
        "fs.list" => refs::listing(&scope, reference, resource.list()?),
        "fs.openRead" => resource.open_read().map_err(String::from),
        "fs.readChunk" => {
            let opened = resource.open_read()?;
            let mut params = ctx.params.clone();
            params["handle"] = opened["handle"].clone();
            if params["offset"].is_null() {
                params["offset"] = json!(0);
            }
            if params["length"].is_null() {
                params["length"] = json!(fs::CHUNK);
            }
            let result = fs::stream_call(&owner, "fs.readOpen", &params);
            let _ = fs::stream_call(&owner, "fs.closeRead", &params);
            result
        }
        "fs.beginWrite" => resource.begin_write(create).map_err(String::from),
        "fs.mkdir" => {
            resource.mkdir()?;
            Ok(Value::Null)
        }
        "fs.remove" => {
            resource.remove()?;
            Ok(Value::Null)
        }
        "fs.rename" => {
            resource.rename(destination.as_ref().ok_or("missing destination")?)?;
            Ok(Value::Null)
        }
        _ => Err("unsupported filesystem method".into()),
    }
}
