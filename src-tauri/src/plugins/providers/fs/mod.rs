//! Core bridge for plugin-owned shared `VfsProvider` implementations.
//!
//! Connection mounts and standalone views differ only in the identity block
//! they send; method names, DTO validation, pagination, stream handling and
//! error mapping are shared. Callers hand in a [`Mount`] built from the mount
//! registry, so no host path ever reaches a plugin.
use serde_json::Value;

const PAGE_LIMIT: usize = 500;
const MAX_PAGES: usize = 1000;
const MAX_ENTRIES: usize = 100_000;

/// Where a mount's plugin identity comes from.
#[derive(Clone, Debug)]
pub enum Source {
    /// Connection-backed mount: the provider gets its settings and secrets.
    Connection(Value),
    /// Standalone view: the provider gets its contribution and view context.
    View {
        contribution_id: String,
        view_context: Value,
    },
}

/// One resolved mount plus the scope its provider calls run under.
#[derive(Clone)]
pub struct Mount {
    pub mount_id: String,
    pub plugin_id: String,
    pub source: Source,
    pub scope: Option<crate::plugins::resource::fs_access::Scope>,
}

/// The plugin that owns a Core mount URI. Works without the desktop feature,
/// so UI-affordance code can consult declared capabilities everywhere.
pub fn mount_plugin_id(path: &str) -> Result<String, String> {
    let (_, source) = crate::plugins::mounts::resolve(path)?;
    Ok(match source {
        crate::plugins::mounts::Source::Connection(connection) => connection.plugin_id,
        crate::plugins::mounts::Source::View(view) => view.descriptor.plugin_id,
    })
}

/// Resolve a Core mount URI (`fiex://mount/<id>/..`) to the provider that
/// owns it. Used by navigation and UI affordance code; protocol calls receive a
/// prepared [`Mount`] instead.
#[cfg(feature = "desktop")]
pub fn mount_for_path(path: &str) -> Result<Mount, String> {
    let (uri, source) = crate::plugins::mounts::resolve(path)?;
    match source {
        crate::plugins::mounts::Source::Connection(connection) => {
            crate::remote::ensure_provider_access(&connection)?;
            Ok(Mount {
                mount_id: uri.mount_id,
                plugin_id: connection.plugin_id.clone(),
                source: Source::Connection(crate::remote::provider_config(&connection)),
                scope: None,
            })
        }
        crate::plugins::mounts::Source::View(view) => Ok(Mount {
            mount_id: uri.mount_id,
            plugin_id: view.descriptor.plugin_id.clone(),
            source: Source::View {
                contribution_id: view.descriptor.contribution_id.clone(),
                view_context: view.descriptor.view_context.clone(),
            },
            scope: Some(view.scope.clone()),
        }),
    }
}

mod endpoint;
mod mount;
mod proxy;
mod streams;
mod types;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

pub use streams::{FsStream, FsWriteStream};
pub use types::{FsEntry, FsKind, FsListing, WriteRequest};
#[cfg(test)]
mod tests;
