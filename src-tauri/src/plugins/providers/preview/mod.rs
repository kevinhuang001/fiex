//! Unified preview registry: built-ins and plugins return the same resolved view.
//!
//! Providers are matched by extension/MIME and priority. Built-ins use a
//! trusted local file; plugins receive a permission-projected `EventContext`
//! and return a package-confined `PreviewView`.
use crate::plugins::contracts::PreviewView;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{PreviewProvider, ProviderRequest};
use serde_json::Value;
use std::{fs, sync::Arc};

mod builtin;
mod native;
mod proxy;
mod session;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}
use crate::plugins::events::{EventRouter, ProviderCandidate, Resolver};
pub use session::session_scope;
use session::UNSUPPORTED;

fn candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Resolver> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let target = fiex_provider_contracts::MatchResource {
        metadata: Some(fiex_provider_contracts::ResourceMetadata {
            kind: if is_dir { "directory" } else { "file" }.into(),
            extension: Some(ext.clone()),
            mime: Some(mime.into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let targets = [target];
    let builtin = native::candidates(&ext, mime, is_dir);
    crate::plugins::provider_registry::candidates("preview.open", &targets, "", builtin)
}

fn resource_size(resource: &ResourceSnapshot) -> Option<u64> {
    resource
        .stat
        .as_ref()
        .map(|stat| stat.len())
        .or_else(|| resource.metadata().and_then(|metadata| metadata.size))
}

fn resource_candidates(resource: &ResourceSnapshot) -> (Vec<Resolver>, bool) {
    let targets = crate::plugins::events::matching::match_resources(std::slice::from_ref(resource));
    let readable = targets.first().is_some_and(|target| {
        target
            .capabilities
            .operations
            .contains(&fiex_provider_contracts::FsOperation::Read)
    });
    let fallback_allowed = native::fallback_matches(&targets);
    let builtin = native::candidates_for(&targets)
        .into_iter()
        .filter(|provider| readable || provider.id == "builtin:fallback")
        .collect();
    let mut providers =
        crate::plugins::provider_registry::candidates("preview.open", &targets, "", builtin);
    // A provider that declares a preview byte ceiling must not be selected for
    // a file above it. This is also the cheapest way to avoid opening a huge
    // archive/Office document only to fail inside the plugin.
    // Local files have OS metadata; virtual files and plugin VFS entries
    // carry their size in the trusted Core ResourceSnapshot metadata instead.
    // Failing to inspect that size bypasses maxBytes for remote previews.
    let size = resource_size(resource);
    providers.retain(|provider| {
        let Some(plugin) = provider.plugin.as_deref() else {
            return true;
        };
        plugin
            .extensions
            .pointer("/preview/maxBytes")
            .and_then(Value::as_u64)
            .is_none_or(|limit| size.is_none_or(|size| size <= limit))
    });
    (providers, fallback_allowed)
}
pub fn resource_candidates_json(display: &str) -> Result<Vec<Value>, String> {
    let resource =
        crate::plugins::resource::resource_context::resolve_from_directory_snapshot(display)?;
    Ok(project_candidates(resource_candidates(&resource).0))
}
fn project_candidates(providers: Vec<Resolver>) -> Vec<Value> {
    providers
        .iter()
        .map(|provider| {
            let mut value = provider.json();
            if let Some(plugin) = provider.plugin.as_deref() {
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
    user_action: crate::core::fs::mapped_access::UserActionContext,
) -> Result<Value, String> {
    resolve_preview(
        crate::plugins::resource::resource_context::resolve_from_directory_snapshot(display)?,
        preferred,
        order,
        user_action,
    )
}
fn resolve_preview(
    resource: ResourceSnapshot,
    preferred: Option<&str>,
    order: Option<&[String]>,
    user_action: crate::core::fs::mapped_access::UserActionContext,
) -> Result<Value, String> {
    let (mut providers, fallback_allowed) = resource_candidates(&resource);
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
    if fallback_allowed {
        providers.push(Resolver::builtin("builtin:fallback", i64::MIN));
    }
    let resource = Arc::new(resource);
    let request = ProviderRequest {
        method: "preview.open".into(),
        params: serde_json::json!({}),
    };
    let providers = providers
        .into_iter()
        .filter_map(|resolver| {
            let implementation: Arc<dyn PreviewProvider> = match resolver.plugin.as_deref() {
                None => native::provider(&resolver.id)?,
                Some(plugin) => Arc::new(
                    proxy::ScopedPreviewProxy::new(
                        plugin.clone(),
                        resource.clone(),
                        resolver.id.clone(),
                    )
                    .ok()?,
                ),
            };
            Some(ProviderCandidate {
                resolver,
                implementation,
            })
        })
        .collect();
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        request.method.clone(),
        request.params.clone(),
        std::slice::from_ref(resource.as_ref()),
        None,
    )
    .with_user_action_context(user_action);
    EventRouter::resolve_shared(
        "preview.open",
        providers,
        &invocation,
        |resolver, provider, context| {
            let result = if resolver.plugin.is_none() {
                let native = native::invocation(
                    &resolver.id,
                    request.method.clone(),
                    request.params.clone(),
                    resource.as_ref(),
                )?
                .with_user_action_context(user_action);
                provider.open(&native)
            } else {
                provider.open(context)
            };
            result.map_err(|error| {
                if error.message == UNSUPPORTED {
                    UNSUPPORTED.to_string()
                } else {
                    error.to_string()
                }
            })
        },
        |_, view| {
            let value = serde_json::to_value(&view).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("preview.open", &value)?;
            Ok(view)
        },
        |view| view.kind != "unsupported",
        move |outcomes| {
            // Every plugin failure is worth keeping: the in-memory error log is
            // shown in Settings, where a bare "no preview provider succeeded"
            // message would hide crashes, handshake and sandbox failures.
            for (provider, result) in &outcomes {
                let (Some(plugin), Err(error)) = (provider.plugin.as_deref(), result) else {
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
                    if provider.plugin.is_some() && error != UNSUPPORTED {
                        return Err(format!("{} preview failed: {error}", provider.name));
                    }
                }
            }
            let (provider, view) = outcomes
                .into_iter()
                .find_map(|(provider, result)| result.ok().map(|view| (provider, view)))
                .ok_or_else(|| "no preview provider succeeded".to_string())?;
            let view = serde_json::to_value(view).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "providerId": provider.id,
                "provider": provider.name,
                "priority": provider.priority,
                "view": view,
            }))
        },
    )
}

#[cfg(test)]
mod tests;
