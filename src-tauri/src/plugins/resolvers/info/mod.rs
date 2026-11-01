//! "Show info" registry: built-in providers plus standard `info.resolve`.
//!
//! Built-in and plugin providers return the same section/row domain types.
//! Core builds an `EventContext` after permission projection, invokes the
//! matching provider, and merges validated sections with native
//! metadata and media probes.
pub use crate::plugins::contracts::{InfoRow, InfoSection};
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::ProviderRequest;
use serde_json::json;
use std::sync::Arc;

fn row(label: impl Into<String>, value: impl Into<String>) -> InfoRow {
    InfoRow {
        label: label.into(),
        value: value.into(),
        copyable: false,
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

/// Builtin and native handlers share ordering, validation and the Info merge policy.
fn resolve_info(resource: ResourceSnapshot) -> Vec<InfoSection> {
    use crate::plugins::events::{EventRouter, Handler};
    let targets = crate::plugins::events::matching::match_resources(std::slice::from_ref(&resource));
    let builtin = native::candidates(&resource);
    let providers = crate::plugins::provider_registry::candidates(
        "info.resolve", &targets, "", builtin,
    );
    let resource = Arc::new(resource);
    let request = ProviderRequest {
        method: "info.resolve".into(),
        params: serde_json::json!({}),
    };
    EventRouter::resolve_typed(
        "info.resolve",
        providers,
        &request,
        |resolver, request| match &resolver.handler {
            Handler::Builtin(id) => {
                native::invoke(id, &resource).map_err(|error| error.to_string())
            }
            Handler::Plugin(plugin) => {
                let request = crate::plugins::events::bind_provider_request(resolver, request)?;
                proxy::ScopedInfoProxy::new((**plugin).clone(), resource.clone())?
                    .resolve_typed(&request)
                    .map_err(|error| error.to_string())
            }
        },
        |provider, mut sections| {
            let value = json!({"sections": &sections});
            crate::plugins::events::validate_result("info.resolve", &value)?;
            for section in &mut sections {
                match &provider.handler {
                    Handler::Plugin(plugin) => {
                        section.id = format!("{}.{}", plugin.id, section.id);
                        section.source = "plugin".into();
                        section.plugin_id = Some(plugin.id.clone());
                    }
                    Handler::Builtin(_) => {
                        section.source = "core".into();
                        section.plugin_id = None;
                    }
                }
                section.provider = provider.name.clone();
            }
            Ok(sections)
        },
        |_| true,
        |outcomes| {
            outcomes
                .into_iter()
                .flat_map(|(provider, result)| match result {
                    Ok(sections) => sections,
                    Err(error) => vec![InfoSection {
                        id: format!(
                            "{}.error",
                            provider.id.strip_prefix("plugin:").unwrap_or(&provider.id)
                        ),
                        title: provider.name.clone(),
                        provider: provider.name,
                        source: if matches!(&provider.handler, Handler::Plugin(_)) {
                            "plugin"
                        } else {
                            "core"
                        }
                        .into(),
                        plugin_id: match provider.handler {
                            Handler::Plugin(p) => Some(p.id),
                            _ => None,
                        },
                        rows: vec![row(
                            crate::i18n::t("错误"),
                            crate::infrastructure::redact::clean(&error, 1500),
                        )],
                    }],
                })
                .collect()
        },
    )
}

/// Remote metadata is matched directly; content providers materialize lazily.
pub fn file_info_resource(display: &str) -> Result<Vec<InfoSection>, String> {
    Ok(resolve_info(ResourceSnapshot::resolve(display)?))
}

#[cfg(test)]
mod tests;
