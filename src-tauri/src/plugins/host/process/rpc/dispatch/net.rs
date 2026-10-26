//! Host-mediated network methods. Opening a stream is the authorization
//! point; handle reads and writes require the current grant without reopening
//! a dialog.
use super::Context;
use crate::plugin_permission_policy::Capability;
use serde_json::Value;

fn request_url_host(params: &Value) -> Result<String, String> {
    let url = params
        .get("url")
        .and_then(Value::as_str)
        .ok_or("missing url")?;
    reqwest::Url::parse(url)
        .map_err(|error| error.to_string())?
        .host_str()
        .map(str::to_owned)
        .ok_or_else(|| "URL has no host".into())
}

pub(super) fn dispatch(ctx: &mut Context<'_>) -> Result<Value, String> {
    let owner = ctx.owner();
    match ctx.method {
        "host.net.http.open" => {
            let host = request_url_host(ctx.params)?;
            ctx.running.ensure_network_target(&host)?;
            crate::plugin_net::http_open(&owner, &ctx.policy, ctx.params)
        }
        "host.net.http.read" => {
            ctx.running.require_permission(Capability::Network)?;
            crate::plugin_net::http_read(&owner, ctx.params)
        }
        "host.net.http.close" => {
            crate::plugin_net::http_close(&owner, ctx.params)?;
            Ok(Value::Null)
        }
        "host.net.http" => {
            let host = request_url_host(ctx.params)?;
            ctx.running.ensure_network_target(&host)?;
            crate::plugin_net::http(&owner, &ctx.policy, ctx.params)
        }
        "host.net.tcp.open" => {
            let host = ctx.arg("host")?;
            ctx.running.ensure_network_target(&host)?;
            crate::plugin_net::tcp_open(&owner, &ctx.policy, ctx.params)
        }
        "host.net.tcp.readBytes" => {
            ctx.running.require_permission(Capability::Network)?;
            crate::plugin_net::tcp_read_bytes(&owner, ctx.params)
        }
        "host.net.tcp.writeBytes" => {
            ctx.running.require_permission(Capability::Network)?;
            crate::plugin_net::tcp_write_bytes(&owner, ctx.params)
        }
        "host.net.tcp.read" => {
            ctx.running.require_permission(Capability::Network)?;
            crate::plugin_net::tcp_read(&owner, ctx.params)
        }
        "host.net.tcp.write" => {
            ctx.running.require_permission(Capability::Network)?;
            crate::plugin_net::tcp_write(&owner, ctx.params)
        }
        "host.net.tcp.shutdown" => {
            crate::plugin_net::tcp_shutdown(&owner, ctx.params)?;
            Ok(Value::Null)
        }
        "host.net.tcp.close" => {
            crate::plugin_net::tcp_close(&owner, ctx.params)?;
            Ok(Value::Null)
        }
        _ => Err("host method not available or permission denied".into()),
    }
}
