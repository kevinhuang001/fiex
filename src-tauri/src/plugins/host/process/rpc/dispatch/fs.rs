//! Filesystem host methods. Every arm validates the operation scope first and
//! only then asks the permission layer, so an out-of-scope path never prompts.
use super::Context;
use crate::plugin_permission_policy::Capability;
use serde_json::{json, Value};

pub(super) fn dispatch(ctx: &mut Context<'_>) -> Result<Value, String> {
    let owner = ctx.owner();
    match ctx.method {
        "host.fs.readChunk" => {
            let path = ctx.arg("path")?;
            ctx.running
                .ensure_fs(&ctx.scope()?, Capability::FilesRead, &path)?;
            crate::plugin_fs_access::read_chunk(
                &ctx.scope()?,
                &path,
                ctx.params
                    .get("offset")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                ctx.params
                    .get("length")
                    .and_then(Value::as_u64)
                    .unwrap_or(262144) as usize,
            )
        }
        "host.fs.openRead" => {
            let path = ctx.arg("path")?;
            ctx.running
                .ensure_fs(&ctx.scope()?, Capability::FilesRead, &path)?;
            crate::plugin_fs_access::open_read(&owner, &ctx.scope()?, &path)
        }
        "host.fs.readOpen" => {
            ctx.running.require_permission(Capability::FilesRead)?;
            crate::plugin_fs_access::read_open(
                &owner,
                &ctx.arg("handle")?,
                ctx.params
                    .get("offset")
                    .and_then(Value::as_u64)
                    .ok_or("missing offset")?,
                ctx.params
                    .get("length")
                    .and_then(Value::as_u64)
                    .ok_or("missing length")?
                    .try_into()
                    .map_err(|_| "invalid length")?,
                ctx.params.get("buffer").and_then(Value::as_str),
            )
        }
        "host.fs.closeRead" => {
            crate::plugin_fs_access::close_read(&owner, &ctx.arg("handle")?)?;
            Ok(Value::Null)
        }
        "host.fs.mkdir" => {
            let path = ctx.arg("path")?;
            ctx.running
                .ensure_fs(&ctx.scope()?, Capability::FoldersCreate, &path)?;
            crate::plugin_fs_access::mkdir(&ctx.scope()?, &path)
        }
        "host.fs.beginWrite" => {
            let path = ctx.arg("path")?;
            let create = ctx
                .params
                .get("create")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            ctx.running.ensure_fs(
                &ctx.scope()?,
                if create {
                    Capability::FilesCreate
                } else {
                    Capability::FilesWrite
                },
                &path,
            )?;
            Ok(json!(crate::plugin_fs_access::begin_write(
                &owner,
                &ctx.scope()?,
                &path,
                create
            )?))
        }
        "host.fs.seekWrite" => {
            let handle = ctx.arg("handle")?;
            ctx.running.require_writer(&owner, &handle)?;
            Ok(json!(crate::plugin_fs_access::seek_write(
                &owner,
                &handle,
                &ctx.arg("kind")?,
                ctx.params
                    .get("offset")
                    .and_then(Value::as_i64)
                    .ok_or("missing seek offset")?
            )?))
        }
        "host.fs.writeChunk" => {
            let handle = ctx.arg("handle")?;
            ctx.running.require_writer(&owner, &handle)?;
            crate::plugin_fs_access::write_chunk(&owner, &handle, &ctx.arg("base64")?)?;
            Ok(Value::Null)
        }
        "host.fs.writeBuffer" => {
            let handle = ctx.arg("handle")?;
            ctx.running.require_writer(&owner, &handle)?;
            crate::plugin_fs_access::write_buffer(
                &owner,
                &handle,
                &ctx.arg("buffer")?,
                ctx.params
                    .get("size")
                    .and_then(Value::as_u64)
                    .ok_or("missing size")?
                    .try_into()
                    .map_err(|_| "invalid size")?,
            )?;
            Ok(Value::Null)
        }
        "host.fs.commit" => {
            let handle = ctx.arg("handle")?;
            ctx.running.require_writer(&owner, &handle)?;
            crate::plugin_fs_access::commit(&owner, &handle)
        }
        "host.fs.abort" => {
            crate::plugin_fs_access::abort(&owner, &ctx.arg("handle")?)?;
            Ok(Value::Null)
        }
        "host.fs.list" => {
            let path = ctx
                .params
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or(".");
            ctx.running
                .ensure_fs(&ctx.scope()?, Capability::FoldersRead, path)?;
            crate::plugin_fs_access::list(&ctx.scope()?, path)
        }
        "host.fs.stat" => {
            let path = ctx.arg("path")?;
            ctx.running
                .ensure_fs(&ctx.scope()?, Capability::Metadata, &path)?;
            crate::plugin_fs_access::stat(&ctx.scope()?, &path)
        }
        "host.fs.statBatch" => {
            let paths = ctx
                .params
                .get("paths")
                .and_then(Value::as_array)
                .ok_or("missing paths")?;
            let scope = ctx.scope()?;
            // Scope is checked per entry. A batch that only contains
            // out-of-scope paths must fail without a permission dialog.
            let in_scope = paths.iter().any(|value| {
                value
                    .as_str()
                    .is_some_and(|path| scope.authorize(path).is_ok())
            });
            if in_scope {
                ctx.running.ensure_permission(Capability::Metadata)?;
            }
            crate::plugin_fs_access::stat_batch(&scope, paths)
        }
        "host.fs.remove" => {
            let path = ctx.arg("path")?;
            let scope = ctx.scope()?;
            // Scope first: an out-of-scope path is never shown in a prompt.
            scope.authorize(&path)?;
            let capability = if crate::plugin_fs_access::is_directory(&scope, &path)? {
                Capability::FoldersDelete
            } else {
                Capability::FilesDelete
            };
            ctx.running.ensure_permission(capability)?;
            crate::plugin_fs_access::remove(&scope, &path)?;
            Ok(Value::Null)
        }
        "host.fs.rename" => {
            let from = ctx.arg("from")?;
            let to = ctx.arg("to")?;
            let scope = ctx.scope()?;
            scope.authorize(&from)?;
            scope.authorize(&to)?;
            let capability = if crate::plugin_fs_access::is_directory(&scope, &from)? {
                Capability::FoldersRename
            } else {
                Capability::FilesRename
            };
            ctx.running.ensure_permission(capability)?;
            crate::plugin_fs_access::rename(&scope, &from, &to)?;
            Ok(Value::Null)
        }
        _ => Err("host method not available or permission denied".into()),
    }
}
