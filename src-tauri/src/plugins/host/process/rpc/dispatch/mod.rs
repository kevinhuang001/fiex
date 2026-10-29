//! Host-method routing. Scope and reference resolution happen here once; the
//! concrete method groups live in `vfs`, `net` and `other` so each family stays
//! small and every permission decision is next to the operation it guards.
use super::{actor::Actor, Running};
use crate::plugins::security::policy::PermissionPolicy;
use serde_json::Value;

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
    let actor = running.actor(params)?;
    if let Some(scope) = &actor.scope {
        scope.authorize_method(method)?;
    }
    if let Some(result) = refs::intercept(running, &actor, method, params)? {
        return Ok(result);
    }
    let mut ctx = Context {
        running,
        actor: &actor,
        method,
        params,
        policy: running.policy(),
    };
    if method.starts_with("fs.") {
        fs::dispatch(&mut ctx)
    } else if method.starts_with("host.net.") || method == "host.oauth.authorize" {
        net::dispatch(&mut ctx)
    } else {
        other::dispatch(&mut ctx)
    }
}
