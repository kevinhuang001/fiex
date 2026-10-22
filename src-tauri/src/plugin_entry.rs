//! Migration adapter for the first three native plugin processes.
//! Backend implementation will move out of the fiex crate incrementally.
use fiex::{git, git_history, git_remote, plugin_sdk, remote};
use serde_json::{json, Value};

fn as_string<'a>(p: &'a Value, name: &str) -> Result<&'a str, plugin_sdk::RpcError> {
    p.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| plugin_sdk::RpcError::invalid_params(format!("missing {name}")))
}
fn arg_vec(p: &Value, name: &str) -> Result<Vec<String>, plugin_sdk::RpcError> {
    let array = p
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| plugin_sdk::RpcError::invalid_params(format!("missing {name}")))?;
    array
        .iter()
        .map(|x| {
            x.as_str()
                .map(str::to_string)
                .ok_or_else(|| plugin_sdk::RpcError::invalid_params("invalid path"))
        })
        .collect()
}
fn wrap<T: serde::Serialize>(result: Result<T, String>) -> Result<Value, plugin_sdk::RpcError> {
    let value = result.map_err(plugin_sdk::RpcError::internal)?;
    serde_json::to_value(value).map_err(|e| plugin_sdk::RpcError::internal(e.to_string()))
}
fn git_dispatch(method: &str, p: Value) -> Result<Value, plugin_sdk::RpcError> {
    let root = || as_string(&p, "root");
    match method {
        "git.status" => Ok(serde_json::to_value(git::status(root()?)).unwrap_or(Value::Null)),
        "git.init" => wrap(git::init(root()?)),
        "git.refs" => wrap(git::refs(root()?)),
        "git.identity" => wrap(Ok(git::identity(root()?))),
        "git.stage" => wrap(git::stage(
            root()?,
            &arg_vec(&p, "paths")?,
            p.get("all").and_then(Value::as_bool).unwrap_or(false),
        )),
        "git.unstage" => wrap(git::unstage(
            root()?,
            &arg_vec(&p, "paths")?,
            p.get("all").and_then(Value::as_bool).unwrap_or(false),
        )),
        "git.commit" => wrap(git::commit(
            root()?,
            as_string(&p, "message")?,
            as_string(&p, "name")?,
            as_string(&p, "email")?,
        )),
        "git.history" => wrap(git_history::history(
            root()?,
            p.get("skip").and_then(Value::as_u64).unwrap_or(0) as usize,
        )),
        "git.browse" => wrap(git_history::browse(root()?, as_string(&p, "reference")?)),
        "git.historyAsset" => wrap(git_history::asset(as_string(&p, "path")?)),
        "git.remoteStatus" => wrap(git_remote::status(root()?)),
        "git.remoteSave" => wrap(git_remote::save(
            root()?,
            as_string(&p, "name")?,
            as_string(&p, "url")?,
        )),
        "git.remoteRemove" => wrap(git_remote::remove(root()?, as_string(&p, "name")?)),
        "git.remoteSync" => wrap(git_remote::sync(
            root()?,
            as_string(&p, "remote")?,
            p.get("push").and_then(Value::as_bool).unwrap_or(false),
            p.get("branch").and_then(Value::as_str),
        )),
        _ => Err(plugin_sdk::RpcError::method_not_found(method)),
    }
}
fn remote_dispatch(kind: &str, method: &str, p: Value) -> Result<Value, plugin_sdk::RpcError> {
    match method {
        "connection.list" => Ok(json!(remote::list_connections()
            .into_iter()
            .filter(|c| c.kind == kind)
            .collect::<Vec<_>>())),
        "connection.save" => {
            let input: remote::ConnectionInput = serde_json::from_value(p)
                .map_err(|e| plugin_sdk::RpcError::invalid_params(e.to_string()))?;
            if input.kind != kind {
                return Err(plugin_sdk::RpcError::invalid_params(
                    "wrong connection kind",
                ));
            }
            wrap(remote::save_connection(input))
        }
        "connection.remove" => wrap(remote::remove_connection(as_string(&p, "id")?)),
        "fs.list" => {
            let id = as_string(&p, "connectionId")?;
            let relative = p.get("relativePath").and_then(Value::as_str).unwrap_or("");
            wrap(tauri::async_runtime::block_on(remote::provider_list(
                kind, id, relative,
            )))
        }
        "fs.download" => wrap(tauri::async_runtime::block_on(remote::download(as_string(
            &p, "path",
        )?))),
        "fs.pinDirectory" => wrap(tauri::async_runtime::block_on(remote::download_dir(
            as_string(&p, "path")?,
        ))),
        "fs.refreshLegacyMirror" => wrap(tauri::async_runtime::block_on(remote::sync_dir(
            as_string(&p, "path")?,
        ))),
        _ => Err(plugin_sdk::RpcError::method_not_found(method)),
    }
}

pub fn run(kind: &str) {
    let id = format!("org.fiex.{kind}");
    if let Err(e) = plugin_sdk::serve(&id, |method, params| {
        if kind == "git" {
            git_dispatch(method, params)
        } else {
            remote_dispatch(kind, method, params)
        }
    }) {
        eprintln!("Fiex {kind} plugin failed: {e}");
    }
}
