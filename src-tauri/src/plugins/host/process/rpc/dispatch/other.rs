//! Host methods outside the filesystem and network families: VFS views,
//! previews, buffers, private storage, diagnostics and command execution.
use super::super::super::spawn::{host_exec, run_command};
use super::Context;
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path};

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
            crate::mounts::open_view(&plugin_id, &params, &scope)
        }
        #[cfg(feature = "desktop")]
        "host.terminal.ready" => {
            crate::desktop::ssh::plugin_terminal_ready(&plugin_id, ctx.params)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "desktop")]
        "host.terminal.output" => {
            crate::desktop::ssh::plugin_terminal_output(&plugin_id, ctx.params)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "desktop")]
        "host.terminal.input" => crate::desktop::ssh::plugin_terminal_input(&plugin_id, ctx.params),
        #[cfg(feature = "desktop")]
        "host.terminal.size" => crate::desktop::ssh::plugin_terminal_size(&plugin_id, ctx.params),
        #[cfg(feature = "desktop")]
        "host.terminal.closed" => {
            crate::desktop::ssh::plugin_terminal_closed(&plugin_id, ctx.params)
        }
        #[cfg(feature = "desktop")]
        "host.terminal.end" => {
            crate::desktop::ssh::plugin_terminal_end(&plugin_id, ctx.params)?;
            Ok(Value::Null)
        }
        #[cfg(not(feature = "desktop"))]
        "host.terminal.ready"
        | "host.terminal.output"
        | "host.terminal.input"
        | "host.terminal.size"
        | "host.terminal.closed"
        | "host.terminal.end" => Err(crate::i18n::t("终端仅在桌面版可用").into()),
        "host.preview.create" => {
            use base64::Engine;
            let name = ctx.arg("name")?;
            if name.is_empty()
                || !Path::new(&name)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                || Path::new(&name).components().count() != 1
            {
                return Err("invalid preview name".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(ctx.arg("base64")?)
                .map_err(|error| error.to_string())?;
            if bytes.len() > 1024 * 1024 {
                return Err("preview too large".into());
            }
            let directory = dirs::cache_dir()
                .ok_or("cache directory unavailable")?
                .join("fiex")
                .join("plugin-previews")
                .join(&plugin_id);
            fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let path = directory.join(&name);
            fs::write(&path, bytes).map_err(|error| error.to_string())?;
            Ok(json!(path.to_string_lossy()))
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
            serde_json::to_value(crate::plugin_ipc::create(&owner, size, writable)?)
                .map_err(|error| error.to_string())
        }
        "host.buffer.map" => {
            let id = ctx.arg("id")?;
            serde_json::to_value(crate::plugin_ipc::map(&owner, &id)?)
                .map_err(|error| error.to_string())
        }
        "host.buffer.release" => {
            let id = ctx.arg("id")?;
            crate::plugin_ipc::release(&owner, &id)?;
            Ok(Value::Null)
        }
        "host.storage.get" => {
            let value =
                crate::plugin_storage::get(&plugin_id, &ctx.arg("scope")?, &ctx.arg("key")?)?;
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
            Ok(json!(crate::plugin_storage::compare_and_set(
                &plugin_id,
                &ctx.arg("scope")?,
                &ctx.arg("key")?,
                expected,
                next
            )?))
        }
        "host.storage.set" => {
            let value = ctx.params.get("value").cloned().ok_or("missing value")?;
            crate::plugin_storage::set(&plugin_id, &ctx.arg("scope")?, &ctx.arg("key")?, value)?;
            Ok(Value::Null)
        }
        "host.storage.delete" => {
            crate::plugin_storage::delete(&plugin_id, &ctx.arg("scope")?, &ctx.arg("key")?)?;
            Ok(Value::Null)
        }
        "host.storage.keys" => Ok(json!(crate::plugin_storage::list_keys(
            &plugin_id,
            &ctx.arg("scope")?
        )?)),
        "host.secret.get" | "host.secret.set" | "host.secret.delete" => {
            let operation = ctx
                .method
                .rsplit('.')
                .next()
                .ok_or("missing secret operation")?;
            crate::plugin_storage::secret_response(
                &plugin_id,
                &ctx.arg("scope")?,
                &ctx.arg("key")?,
                operation,
                ctx.params["value"].as_str(),
            )
        }
        "host.secret.available" => Ok(json!(crate::plugin_storage::secret_available())),
        // Diagnostics only: no permission gate, capped and best effort.
        "host.error.report" => {
            let code = ctx.params.get("code").and_then(Value::as_str).unwrap_or("");
            let message = ctx
                .params
                .get("message")
                .and_then(Value::as_str)
                .ok_or("missing message")?;
            let detail = ctx.params.get("detail").and_then(Value::as_str);
            crate::plugin_errors::report_limited(&plugin_id, code, message, detail);
            Ok(Value::Null)
        }
        "host.exec" => {
            ctx.running.ensure_command(&ctx.arg("command")?)?;
            host_exec(&plugin_id, &ctx.running.declared, ctx.params)
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
