//! "Show info" registry: built-in providers plus standard `info.resolve`.
//!
//! Built-in and plugin providers return the same section/row domain types.
//! Core builds an `EventContext` after permission projection, invokes the
//! matching provider, and merges validated sections with native
//! metadata and media probes.
pub use crate::plugins::contracts::{InfoRow, InfoSection};
use crate::plugins::resource::resource_context::ResourceSnapshot;

fn row(label: impl Into<String>, value: impl Into<String>) -> InfoRow {
    InfoRow {
        label: label.into(),
        value: value.into(),
        copyable: false,
    }
}

mod media;
mod metadata;
pub use media::media_sections;
use metadata::metadata_section;
/// Builtin and native handlers share ordering, validation and the Info merge policy.
fn resolve_info(display: &str, resource: ResourceSnapshot) -> Vec<InfoSection> {
    use crate::plugins::events::{EventRouter, Handler, Resolver};
    let metadata =
        crate::plugins::events::resource_metadata(std::slice::from_ref(&resource)).remove(0);
    let mut builtin = vec![Resolver::builtin("core.metadata", 1000)];
    if !resource.is_dir
        && (resource.mime.starts_with("audio/") || resource.mime.starts_with("video/"))
    {
        builtin.push(Resolver::builtin("core.media", 100));
    }
    let providers = EventRouter::providers("info.resolve", &[metadata], "", builtin);
    EventRouter::resolve(
        "info.resolve",
        providers,
        |_, id| {
            let sections = match id {
                "core.metadata" => vec![metadata_section(
                    display,
                    &resource.local,
                    resource.stat.as_ref(),
                    resource.metadata(),
                )],
                "core.media" => media_sections(&resource.export_path()?),
                _ => return Err("unknown info builtin".into()),
            };
            Ok(serde_json::json!({"sections":sections}))
        },
        |plugin| {
            crate::plugins::events::invoke_resources(
                plugin,
                "info.resolve",
                serde_json::json!({}),
                std::slice::from_ref(&resource),
                None,
            )
        },
        |provider, value| {
            let mut sections: Vec<InfoSection> =
                serde_json::from_value(value["sections"].clone()).map_err(|e| e.to_string())?;
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
    Ok(resolve_info(display, ResourceSnapshot::resolve(display)?))
}

#[cfg(test)]
mod tests;
