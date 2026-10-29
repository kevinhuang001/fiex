//! Unified preview registry: built-ins and plugins return the same resolved view.
//!
//! Providers are matched by extension/MIME and priority. Built-ins use a
//! trusted local file; plugins receive a permission-projected `EventContext`
//! and return a package-confined `PreviewView`.
use crate::plugins::contracts::PreviewView;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use serde_json::Value;
use std::fs;

mod builtin;
mod session;
use crate::plugins::events::{EventRouter, Handler, Resolver};
use builtin::{builtin_candidates, builtin_view, Builtin};
pub use session::session_scope;
use session::{open_plugin_resource, UNSUPPORTED};

fn candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Resolver> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let builtin = builtin_candidates(&ext, mime, is_dir)
        .into_iter()
        .map(|b| Resolver::builtin(b.id(), b.priority()))
        .collect();
    EventRouter::providers(
        "preview.open",
        &[
            serde_json::json!({"extension":ext,"mime":mime,"kind":if is_dir{"directory"}else{"file"}}),
        ],
        "",
        builtin,
    )
}

fn resource_candidates(resource: &ResourceSnapshot) -> Vec<Resolver> {
    let metadata = crate::plugins::events::resource_metadata(std::slice::from_ref(resource));
    let readable = metadata[0]["operations"]
        .as_array()
        .is_some_and(|ops| ops.iter().any(|op| op == "read"));
    let builtin = builtin_candidates(&resource.ext, &resource.mime, resource.is_dir)
        .into_iter()
        .filter(|kind| readable || *kind == Builtin::Fallback)
        .map(|kind| Resolver::builtin(kind.id(), kind.priority()))
        .collect();
    let mut providers = EventRouter::providers("preview.open", &metadata, "", builtin);
    // A provider that declares a preview byte ceiling must not be selected for
    // a file above it. This is also the cheapest way to avoid opening a huge
    // archive/Office document only to fail inside the plugin.
    let size = resource.stat.as_ref().map(|stat| stat.len());
    providers.retain(|provider| {
        let crate::plugins::events::Handler::Plugin(plugin) = &provider.handler else {
            return true;
        };
        plugin
            .extensions
            .pointer("/preview/maxBytes")
            .and_then(Value::as_u64)
            .is_none_or(|limit| size.is_none_or(|size| size <= limit))
    });
    providers
}
pub fn resource_candidates_json(display: &str) -> Result<Vec<Value>, String> {
    let resource = ResourceSnapshot::resolve(display)?;
    Ok(project_candidates(resource_candidates(&resource)))
}
fn project_candidates(providers: Vec<Resolver>) -> Vec<Value> {
    providers
        .iter()
        .map(|provider| {
            let mut value = provider.json();
            if let crate::plugins::events::Handler::Plugin(plugin) = &provider.handler {
                if crate::plugins::host::preview_loading_entry(&plugin.extensions).is_some() {
                    value["loading"] = Value::Bool(true);
                }
            }
            value
        })
        .collect()
}

/// Ordered provider list for the source switcher. User ordering and the
/// disabled list remain presentation preferences. A plugin that declares a
/// loading page advertises it here so the UI can resolve the document before
/// invoking `preview.open`.
pub fn candidates_json(ext: &str, mime: &str, is_dir: bool) -> Vec<Value> {
    project_candidates(candidates(ext, mime, is_dir))
}

pub fn open_resource(
    display: &str,
    preferred: Option<&str>,
    order: Option<&[String]>,
) -> Result<Value, String> {
    resolve_preview(ResourceSnapshot::resolve(display)?, preferred, order)
}
fn resolve_preview(
    resource: ResourceSnapshot,
    preferred: Option<&str>,
    order: Option<&[String]>,
) -> Result<Value, String> {
    let mut providers = resource_candidates(&resource);
    if let Some(order) = order {
        providers.retain(|p| order.contains(&p.id));
        providers.sort_by_key(|p| {
            order
                .iter()
                .position(|id| id == &p.id)
                .unwrap_or(usize::MAX)
        });
    }
    if let Some(id) = preferred {
        let index = providers
            .iter()
            .position(|p| p.id == id)
            .ok_or("unknown preview provider")?;
        let first = providers.remove(index);
        providers.insert(0, first);
    }
    let preferred_id = preferred.map(str::to_owned);
    // The final fallback is a label-only renderer; it never reads/downloads content.
    providers.retain(|p| p.id != "builtin:fallback");
    providers.push(Resolver::builtin("builtin:fallback", i64::MIN));
    EventRouter::resolve(
        "preview.open",
        providers,
        |_, id| {
            let kind = [
                Builtin::Text,
                Builtin::Image,
                Builtin::Media,
                Builtin::Pdf,
                Builtin::Fallback,
            ]
            .into_iter()
            .find(|b| b.id() == id)
            .ok_or("unknown preview builtin")?;
            let path = if kind == Builtin::Fallback {
                resource.local.clone()
            } else {
                resource.export_path()?
            };
            builtin_view(kind, &path, &resource.display)
        },
        |plugin| {
            open_plugin_resource(&resource, plugin).map(|response| {
                let mut view = response["view"].clone();
                if let Some(object) = view.as_object_mut() {
                    for key in [
                        "sessionId",
                        "context",
                        "pluginId",
                        "contributionId",
                        "timeoutMs",
                    ] {
                        object.insert(key.into(), response[key].clone());
                    }
                }
                view
            })
        },
        |provider, mut view| {
            if let Some(object) = view.as_object_mut() {
                object.insert("providerId".into(), Value::String(provider.id.clone()));
                object.insert("provider".into(), Value::String(provider.name.clone()));
            }
            Ok(view)
        },
        |_| true,
        move |outcomes| {
            // Every plugin failure is worth keeping: the in-memory error log is
            // shown in Settings, where a bare "no preview provider succeeded"
            // message would hide crashes, handshake and sandbox failures.
            for (provider, result) in &outcomes {
                let (Handler::Plugin(plugin), Err(error)) = (&provider.handler, result) else {
                    continue;
                };
                if error != UNSUPPORTED {
                    let _ = crate::plugins::errors::report(
                        &plugin.id,
                        "host",
                        &format!("{}: {error}", provider.name),
                        Some(error.as_str()),
                    );
                }
            }
            // Keep the provider the user selected. Only an explicit
            // "unsupported" answer may fall through to the next provider (the
            // label-only builtin included); a crash or transport failure is
            // surfaced as an error so the UI never silently swaps providers.
            if let Some(id) = preferred_id.as_deref() {
                if let Some((provider, Err(error))) =
                    outcomes.iter().find(|(provider, _)| provider.id == id)
                {
                    if matches!(&provider.handler, Handler::Plugin(_)) && error != UNSUPPORTED {
                        return Err(format!("{} preview failed: {error}", provider.name));
                    }
                }
            }
            outcomes.into_iter().find_map(|(provider,result)|result.ok().map(|view|serde_json::json!({"providerId":provider.id,"provider":provider.name,"priority":provider.priority,"view":view}))).ok_or_else(||"no preview provider succeeded".into())
        },
    )
}

#[cfg(test)]
mod tests;
