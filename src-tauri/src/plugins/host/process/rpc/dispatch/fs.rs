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
        "host.fs.readOpen"
            | "host.fs.closeRead"
            | "host.fs.seekWrite"
            | "host.fs.writeChunk"
            | "host.fs.writeBuffer"
            | "host.fs.commit"
            | "host.fs.abort"
    ) {
        // Look up the actor-bound handle before inspecting grants. Guessing a
        // handle owned by another task can never cause an authorization prompt.
        let permission = fs::stream_permission(&owner, &ctx.arg("handle")?)?;
        if !matches!(ctx.method, "host.fs.closeRead" | "host.fs.abort") {
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
    if ctx.method == "host.fs.stat"
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
        "host.fs.stat" => (FsOperation::Stat, Capability::Metadata),
        "host.fs.list" => (FsOperation::List, Capability::FoldersRead),
        "host.fs.openRead" | "host.fs.readChunk" => (FsOperation::Read, Capability::FilesRead),
        "host.fs.beginWrite" => (
            FsOperation::Write,
            if create {
                Capability::FilesCreate
            } else {
                Capability::FilesWrite
            },
        ),
        "host.fs.mkdir" => (FsOperation::Mkdir, Capability::FoldersCreate),
        "host.fs.remove" => (
            FsOperation::Remove,
            if resource.is_dir()? {
                Capability::FoldersDelete
            } else {
                Capability::FilesDelete
            },
        ),
        "host.fs.trash" => (
            FsOperation::Trash,
            if resource.is_dir()? {
                Capability::FoldersDelete
            } else {
                Capability::FilesDelete
            },
        ),
        "host.fs.rename" => (
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
    let destination = if ctx.method == "host.fs.rename" {
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
        "host.fs.stat" => Ok(refs::metadata(&resource.stat()?)),
        "host.fs.list" => refs::listing(&scope, reference, resource.list()?),
        "host.fs.openRead" => resource.open_read().map_err(String::from),
        "host.fs.readChunk" => {
            let opened = resource.open_read()?;
            let mut params = ctx.params.clone();
            params["handle"] = opened["handle"].clone();
            if params["offset"].is_null() {
                params["offset"] = json!(0);
            }
            if params["length"].is_null() {
                params["length"] = json!(fs::CHUNK);
            }
            let result = fs::stream_call(&owner, "host.fs.readOpen", &params);
            let _ = fs::stream_call(&owner, "host.fs.closeRead", &params);
            result
        }
        "host.fs.beginWrite" => resource.begin_write(create).map_err(String::from),
        "host.fs.mkdir" => {
            resource.mkdir()?;
            Ok(Value::Null)
        }
        "host.fs.remove" => {
            resource.remove()?;
            Ok(Value::Null)
        }
        "host.fs.trash" => {
            resource.move_to_trash()?;
            Ok(Value::Null)
        }
        "host.fs.rename" => {
            resource.rename(destination.as_ref().ok_or("missing destination")?)?;
            Ok(Value::Null)
        }
        _ => Err("unsupported filesystem method".into()),
    }
}
