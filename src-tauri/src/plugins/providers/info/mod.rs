//! "Show info" registry: built-in providers plus standard `info.resolve`.
//!
//! Built-in and plugin providers return the same section/row domain types.
//! Core builds an `EventContext` after permission projection, invokes the
//! matching provider, and merges validated sections with native
//! metadata and media probes.
pub use crate::plugins::contracts::{InfoRow, InfoSection};
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{InfoProvider, ProviderRequest};
use serde_json::json;
use std::sync::Arc;

fn row(label: impl Into<String>, value: impl Into<String>) -> InfoRow {
    InfoRow {
        label: label.into(),
        value: value.into(),
        copyable: false,
    }
}

fn provider_error_section(provider: &crate::plugins::events::Resolver, error: &str) -> InfoSection {
    InfoSection {
        id: format!(
            "{}.error",
            provider.id.strip_prefix("plugin:").unwrap_or(&provider.id)
        ),
        title: provider.name.clone(),
        provider: provider.name.clone(),
        source: if provider.plugin.is_some() {
            "plugin"
        } else {
            "core"
        }
        .into(),
        plugin_id: provider.plugin.as_deref().map(|plugin| plugin.id.clone()),
        rows: vec![row(
            crate::i18n::t("错误"),
            crate::infrastructure::redact::clean(error, 1500),
        )],
    }
}

mod media;
mod metadata;
mod native;
mod proxy;
pub use media::media_sections;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

#[cfg(feature = "desktop")]
pub(crate) fn calculate_directory_size(
    path: &std::path::Path,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<u64, String> {
    metadata::directory_size_with_cancel(path, cancelled)
}

#[cfg(feature = "desktop")]
pub(crate) fn file_info_snapshot_with_partial(
    resource: ResourceSnapshot,
    user_action: crate::core::fs::mapped_access::UserActionContext,
    on_partial: impl FnMut(Vec<InfoSection>),
) -> Vec<InfoSection> {
    resolve_info_with_partial(resource, Some(user_action), on_partial)
}

/// Builtin and native handlers share ordering, validation and the Info merge policy.
fn resolve_info(resource: ResourceSnapshot) -> Vec<InfoSection> {
    resolve_info_with_partial(resource, None, |_| {})
}

fn resolve_info_with_partial(
    resource: ResourceSnapshot,
    user_action: Option<crate::core::fs::mapped_access::UserActionContext>,
    mut on_partial: impl FnMut(Vec<InfoSection>),
) -> Vec<InfoSection> {
    use crate::plugins::events::{EventRouter, ProviderCandidate};
    let targets =
        crate::plugins::events::matching::match_resources(std::slice::from_ref(&resource));
    let builtin = native::candidates(&resource);
    let providers =
        crate::plugins::provider_registry::candidates("info.resolve", &targets, "", builtin);
    let resource = Arc::new(resource);
    let request = ProviderRequest {
        method: "info.resolve".into(),
        params: serde_json::json!({}),
    };
    let providers = providers
        .into_iter()
        .filter_map(|resolver| {
            let implementation: Arc<dyn InfoProvider> = match resolver.plugin.as_deref() {
                None => native::provider(&resolver.id)?,
                Some(plugin) => Arc::new(
                    proxy::ScopedInfoProxy::new(
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
    );
    let invocation = match user_action {
        Some(user_action) => invocation.with_user_action_context(user_action),
        None => invocation,
    };
    EventRouter::resolve_merge_parallel_with_partial(
        "info.resolve",
        providers,
        &invocation,
        |resolver, provider, context| {
            if resolver.plugin.is_none() {
                let mut native = native::invocation(
                    &resolver.id,
                    request.method.clone(),
                    request.params.clone(),
                    resource.as_ref(),
                )?;
                if let Some(user_action) = user_action {
                    native = native.with_user_action_context(user_action);
                }
                provider.resolve(&native).map_err(|error| error.to_string())
            } else {
                provider.resolve(context).map_err(|error| error.to_string())
            }
        },
        |provider, mut sections| {
            let value = json!({"sections": &sections});
            crate::plugins::events::validate_result("info.resolve", &value)?;
            for section in &mut sections {
                match provider.plugin.as_deref() {
                    Some(plugin) => {
                        section.id = format!("{}.{}", plugin.id, section.id);
                        section.source = "plugin".into();
                        section.plugin_id = Some(plugin.id.clone());
                    }
                    None => {
                        section.source = "core".into();
                        section.plugin_id = None;
                    }
                }
                section.provider = provider.name.clone();
            }
            Ok(sections)
        },
        |provider, result| {
            let sections = match result {
                Ok(sections) => sections.clone(),
                Err(error) => vec![provider_error_section(provider, error)],
            };
            if !sections.is_empty() {
                on_partial(sections);
            }
        },
        |outcomes| {
            outcomes
                .into_iter()
                .flat_map(|(provider, result)| match result {
                    Ok(sections) => sections,
                    Err(error) => vec![provider_error_section(&provider, &error)],
                })
                .collect()
        },
    )
}

/// Remote metadata is matched directly; content providers materialize lazily.
pub fn file_info_resource(display: &str) -> Result<Vec<InfoSection>, String> {
    Ok(resolve_info(
        crate::plugins::resource::resource_context::resolve(display)?,
    ))
}

#[cfg(test)]
mod tests;
