//! Host-method routing. Scope and reference resolution happen here once; the
//! concrete method groups live in `vfs`, `net` and `other` so each family stays
//! small and every permission decision is next to the operation it guards.
use super::{actor::Actor, Running};
use crate::plugins::security::policy::{Capability, PermissionPolicy};
use serde_json::{json, Value};

mod fs;
mod net;
mod other;
mod refs;
mod task_routes;

pub(super) fn host_method(running: &Running, method: &str, p: &Value) -> Result<Value, String> {
    task_routes::host_method(running, method, p)
}

pub(super) struct Context<'a> {
    pub(super) running: &'a Running,
    pub(super) actor: &'a Actor,
    pub(super) method: &'a str,
    pub(super) params: &'a Value,
    pub(super) policy: PermissionPolicy<'a>,
}

impl Context<'_> {
    pub(super) fn arg(&self, key: &str) -> Result<String, String> {
        self.params
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("missing {key}"))
    }

    pub(super) fn owner(&self) -> String {
        self.actor.owner.clone()
    }

    pub(super) fn scope(&self) -> Result<crate::plugins::resource::fs_access::Scope, String> {
        self.actor
            .scope
            .clone()
            .ok_or_else(|| "no task directory".to_string())
    }

    pub(super) fn scope_path(&self) -> Option<std::path::PathBuf> {
        self.actor
            .scope
            .as_ref()
            .map(|scope| scope.path().to_path_buf())
    }
}

pub(super) fn run(running: &Running, method: &str, params: &Value) -> Result<Value, String> {
    let mut actor = running.actor(params)?;
    if let Some(scope) = &actor.scope {
        scope.authorize_method(method)?;
    }
    if let Some(result) = refs::intercept(running, &actor, method, params)? {
        return Ok(result);
    }
    let policy = running.policy();
    let mut named_params;
    let params = if method == "host.fs.rename" {
        let scope = actor.scope.as_ref().ok_or("no operation scope")?;
        let destination = params.get("destination").ok_or("missing destination ref")?;
        named_params = params.clone();
        named_params["from"] =
            json!(scope.resolve_reference(params.get("ref").ok_or("missing source ref")?)?);
        named_params["to"] = json!(scope.resolve_reference(destination)?);
        &named_params
    } else {
        params
    };
    let mut remote_params;
    let p = if method.starts_with("host.fs.") {
        let remote = actor
            .scope
            .as_ref()
            .map(|scope| scope.remote_target(params))
            .transpose()?
            .flatten();
        if let Some(resource) = remote {
            if method == "host.fs.stat" {
                running.ensure_permission(Capability::Metadata)?;
                let mut value =
                    serde_json::to_value(resource.metadata()).map_err(|e| e.to_string())?;
                value["path"] = json!(resource.display);
                value["isDir"] = json!(resource.is_dir);
                value["readonly"] = json!(resource.read_only);
                value["symlink"] = json!(false);
                for key in ["size", "modified"] {
                    if value[key].is_null() {
                        value[key] = json!(0);
                    }
                }
                return Ok(refs::metadata(&value));
            }
            let capability = match method {
                "host.fs.openRead" | "host.fs.readChunk" => Capability::FilesRead,
                "host.fs.list" => Capability::FoldersRead,
                "host.fs.beginWrite" => {
                    if params
                        .get("create")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        Capability::FilesCreate
                    } else {
                        Capability::FilesWrite
                    }
                }
                "host.fs.remove" => {
                    if resource.is_dir {
                        Capability::FoldersDelete
                    } else {
                        Capability::FilesDelete
                    }
                }
                "host.fs.rename" => {
                    if resource.is_dir {
                        Capability::FoldersRename
                    } else {
                        Capability::FilesRename
                    }
                }
                "host.fs.mkdir" => Capability::FoldersCreate,
                _ => return Err("unsupported remote resource operation".into()),
            };
            running.ensure_permission(capability)?;
            if method == "host.fs.list" {
                let entries = if crate::remote::is_remote(&resource.display) {
                    futures::executor::block_on(crate::remote::list_listing(
                        &resource.display,
                        true,
                        &[],
                    ))?
                    .ok_or("remote directory unavailable")?
                    .entries
                } else {
                    crate::plugins::vfs::list(&resource.display, true, &[])?.entries
                };
                let values = entries
                    .into_iter()
                    .map(|e| {
                        json!({
                            "path": e.path,
                            "name": e.name,
                            "isDir": e.is_dir,
                            "size": e.size,
                            "modified": e.modified,
                            "extension": e.extension,
                            "mime": e.mime,
                            "readonly": e.readonly,
                            "symlink": e.is_symlink
                        })
                    })
                    .collect::<Vec<_>>();
                return refs::listing(
                    actor.scope.as_ref().ok_or("no operation scope")?,
                    &params["ref"],
                    json!(values),
                );
            }
            let local = resource.materialize()?;
            let directory = local.parent().ok_or("remote target has no parent")?;
            let mut scope = crate::plugins::resource::fs_access::Scope::selected(
                directory,
                std::slice::from_ref(&local),
            )?;
            let target = json!({
                "ref": params.get("ref").cloned().unwrap_or_else(|| json!(resource.reference)),
                "metadata": resource.metadata()
            });
            scope.bind_resource(&target, Some(&local))?;
            actor.scope = Some(scope);
            remote_params = params.clone();
            remote_params["path"] = json!(local.to_string_lossy());
            &remote_params
        } else {
            params
        }
    } else {
        params
    };
    let mut resolved;
    let p = if let Some(reference) = p.get("ref") {
        let scope = actor.scope.as_ref().ok_or("no operation scope")?;
        if method == "host.fs.stat" && scope.resolve_reference(reference).is_err() {
            if let Some(metadata) = scope.metadata_reference(reference)? {
                running.ensure_permission(Capability::Metadata)?;
                return Ok(refs::metadata(metadata));
            }
        }
        resolved = p.clone();
        resolved["path"] = json!(scope.resolve_reference(reference)?);
        &resolved
    } else {
        p
    };
    let actor = &actor;
    let mut ctx = Context {
        running,
        actor,
        method,
        params: p,
        policy,
    };
    if method.starts_with("host.fs.") {
        let result = fs::dispatch(&mut ctx)?;
        match method {
            "host.fs.stat" => Ok(refs::metadata(&result)),
            "host.fs.list" => refs::listing(
                actor.scope.as_ref().ok_or("no operation scope")?,
                &params["ref"],
                result,
            ),
            _ => Ok(result),
        }
    } else if method.starts_with("host.net.") || method == "host.oauth.authorize" {
        net::dispatch(&mut ctx)
    } else {
        other::dispatch(&mut ctx)
    }
}
