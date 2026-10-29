//! Host methods outside the filesystem and network families: VFS views,
//! previews, buffers, private storage, diagnostics and command execution.
use super::super::super::spawn::run_command;
use super::Context;
use serde_json::{json, Value};

pub(super) fn dispatch(ctx: &mut Context<'_>) -> Result<Value, String> {
    let owner = ctx.owner();
    let plugin_id = ctx.running.plugin_id().to_string();
    match ctx.method {
        "host.vfs.openView" => {
            let scope = ctx.scope()?;
            let mut params = ctx.params.clone();
            params["resourceRoot"] = json!(scope.resolve_reference(
                ctx.params
                    .get("resourceRoot")
                    .ok_or("missing resource ref")?
            )?);
            crate::plugins::mounts::open_view(&plugin_id, &params, &scope)
        }
        "host.buffer.create" => {
            let size = ctx
                .params
                .get("size")
                .and_then(Value::as_u64)
                .ok_or("missing size")? as usize;
            let writable = ctx
                .params
                .get("writable")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            serde_json::to_value(crate::plugins::ipc::create(&owner, size, writable)?)
                .map_err(|error| error.to_string())
        }
        "host.buffer.map" => {
            let id = ctx.arg("id")?;
            serde_json::to_value(crate::plugins::ipc::map(&owner, &id)?)
                .map_err(|error| error.to_string())
        }
        "host.buffer.release" => {
            let id = ctx.arg("id")?;
            crate::plugins::ipc::release(&owner, &id)?;
            Ok(Value::Null)
        }
        "host.storage.get" => {
            let value =
                crate::plugins::storage::get(&plugin_id, &ctx.arg("scope")?, &ctx.arg("key")?)?;
            Ok(json!({"found":value.is_some(),"value":value}))
        }
        "host.storage.compareAndSet" => {
            let expected = if ctx.params["expectedExists"]
                .as_bool()
                .ok_or("missing expectedExists")?
            {
                Some(
                    ctx.params
                        .get("expected")
                        .cloned()
                        .ok_or("missing expected")?,
                )
            } else {
                None
            };
            let next = if ctx.params["delete"]
                .as_bool()
                .ok_or("missing delete flag")?
            {
                None
            } else {
                Some(ctx.params.get("value").cloned().ok_or("missing value")?)
            };
            Ok(json!(crate::plugins::storage::compare_and_set(
                &plugin_id,
                &ctx.arg("scope")?,
                &ctx.arg("key")?,
                expected,
                next
            )?))
        }
        "host.storage.set" => {
            let value = ctx.params.get("value").cloned().ok_or("missing value")?;
            crate::plugins::storage::set(&plugin_id, &ctx.arg("scope")?, &ctx.arg("key")?, value)?;
            Ok(Value::Null)
        }
        "host.storage.delete" => {
            crate::plugins::storage::delete(&plugin_id, &ctx.arg("scope")?, &ctx.arg("key")?)?;
            Ok(Value::Null)
        }
        "host.storage.keys" => Ok(json!(crate::plugins::storage::list_keys(
            &plugin_id,
            &ctx.arg("scope")?
        )?)),
        "host.secret.get" | "host.secret.set" | "host.secret.delete" => {
            let operation = ctx
                .method
                .rsplit('.')
                .next()
                .ok_or("missing secret operation")?;
            crate::plugins::storage::secret_response(
                &plugin_id,
                &ctx.arg("scope")?,
                &ctx.arg("key")?,
                operation,
                ctx.params["value"].as_str(),
            )
        }
        "host.secret.available" => Ok(json!(crate::plugins::storage::secret_available())),
        // Diagnostics only: no permission gate, capped and best effort.
        "host.error.report" => {
            let code = ctx.params.get("code").and_then(Value::as_str).unwrap_or("");
            let message = ctx
                .params
                .get("message")
                .and_then(Value::as_str)
                .ok_or("missing message")?;
            let detail = ctx.params.get("detail").and_then(Value::as_str);
            crate::plugins::errors::report_limited(&plugin_id, code, message, detail);
            Ok(Value::Null)
        }
        "host.process.run" => {
            ctx.running.ensure_command(&ctx.arg("command")?)?;
            let scope = ctx.scope()?;
            let mut params = ctx.params.clone();
            if let Some(args) = params["args"].as_array_mut() {
                for arg in args {
                    if let Some(encoded) =
                        arg.as_str().and_then(|text| text.strip_prefix("fiex-ref:"))
                    {
                        let reference: Value = serde_json::from_str(encoded)
                            .map_err(|_| "invalid resource argument")?;
                        *arg = json!(scope.resolve_reference(&reference)?);
                    }
                }
            }
            let directory = if let Some(reference) = ctx.params.get("cwd") {
                Some(std::path::PathBuf::from(
                    scope.resolve_reference(reference)?,
                ))
            } else {
                ctx.scope_path()
            };
            if ctx.params.get("cwd").is_some() {
                if let Some(directory) = &directory {
                    scope.authorize(&directory.to_string_lossy())?;
                }
            }
            run_command(
                &plugin_id,
                &ctx.running.declared,
                &params,
                directory.as_deref(),
            )
        }
        _ => Err("host method not available or permission denied".into()),
    }
}
