//! Builtin menu policy. The frontend receives data, never provider closures.
use super::file_ops::{self, Selection};
use crate::plugins::events::Resolver;
use crate::plugins::resource::file_capabilities::FileCapabilities;
use fiex_provider_contracts::{MenuExecuteResult, MenuItem};
use serde_json::Value;

#[derive(Clone)]
pub(super) struct Context {
    pub directory: String,
    pub directory_entry: Option<crate::core::files::Entry>,
    pub paths: Vec<String>,
    pub entries: Vec<crate::core::files::Entry>,
    pub read_only: bool,
    all_local: bool,
    capabilities: FileCapabilities,
}

/// Host-only actions require a direct local FS resource. A location type
/// of "fs" alone is not enough: raw USBFS is also FS but must not be
/// accidentally passed to local operating-system commands.
fn plain_local(path: &str) -> bool {
    let (view, storage) = crate::plugins::match_rules::typed_locations_for(path);
    matches!((view, storage),
        (Some(view), Some(storage))
            if view == storage && storage.provider_id == "core:local-fs")
}

impl Context {
    pub fn load(directory: &str, paths: &[String]) -> Result<Self, String> {
        // All storage kinds resolve through ResourceBroker's trusted scope,
        // rather than a second URI-sensitive routing table in ContextMenu.
        fn entry(path: &str) -> Result<crate::core::files::Entry, String> {
            crate::core::fs::trusted(path, "core:context-menu")?
                .desktop_entry()
                .map_err(String::from)
        }
        let entries = paths
            .iter()
            .map(|path| entry(path))
            .collect::<Result<Vec<_>, _>>()?;
        // Read-only status comes from the selected storage provider's entry,
        // including Core-native views. The menu does not special-case a view.
        let directory_entry = entry(directory)?;
        let read_only = directory_entry.readonly;
        // A blank-canvas menu has no selection: capabilities come from the
        // directory itself so "new folder/file" follows the same grant.
        let mut capabilities = if paths.is_empty() {
            crate::plugins::resource::file_capabilities::for_path(directory)
        } else {
            crate::plugins::resource::file_capabilities::for_selection(paths, &entries)
        };
        if read_only {
            capabilities = capabilities.intersect(
                crate::plugins::resource::file_capabilities::FileCapabilities::READ_ONLY,
            );
        }
        let all_local = plain_local(directory) && paths.iter().all(|path| plain_local(path));
        Ok(Self {
            directory: directory.into(),
            directory_entry: Some(directory_entry),
            paths: paths.to_vec(),
            entries,
            read_only,
            all_local,
            capabilities,
        })
    }

    pub(super) fn selection(&self) -> Selection<'_> {
        Selection {
            paths: &self.paths,
            entries: &self.entries,
            read_only: self.read_only,
            all_local: self.all_local,
        }
    }

    pub(super) fn capabilities(&self) -> FileCapabilities {
        self.capabilities
    }

    /// Tests build menus without touching the mount registry.
    #[cfg(test)]
    pub(super) fn for_test(
        directory: &str,
        paths: Vec<String>,
        entries: Vec<crate::core::files::Entry>,
        read_only: bool,
    ) -> Self {
        let capabilities =
            crate::plugins::resource::file_capabilities::for_selection(&paths, &entries);
        Self::for_test_caps(directory, paths, entries, read_only, capabilities)
    }

    /// Tests pin the capability snapshot to exercise menu gating directly.
    #[cfg(test)]
    pub(super) fn for_test_caps(
        directory: &str,
        paths: Vec<String>,
        entries: Vec<crate::core::files::Entry>,
        read_only: bool,
        capabilities: FileCapabilities,
    ) -> Self {
        let all_local = plain_local(directory) && paths.iter().all(|path| plain_local(path));
        Self {
            directory: directory.into(),
            directory_entry: None,
            paths,
            entries,
            read_only,
            all_local,
            capabilities,
        }
    }
}

/// Only groups this selection can actually use become menu providers.
pub(super) fn retain_unclaimed(ctx: &Context, items: Vec<MenuItem>) -> Vec<MenuItem> {
    let claimed = super::native_views::claimed_actions(&ctx.directory, &ctx.paths);
    items
        .into_iter()
        .filter(|item| !claimed.iter().any(|id| item.id == *id))
        .collect()
}

/// Native and plugin contributions participate in the same EventRouter.
/// A native view claims only its OS-specific action identifiers, not the
/// entire menu. Generic providers still contribute independent actions.
pub(super) fn providers(ctx: &Context) -> Vec<Resolver> {
    let mut candidates = super::native_views::providers(&ctx.directory, &ctx.paths);
    candidates.extend(file_ops::providers(ctx));
    candidates
}

#[cfg(test)]
pub(super) fn resolve(id: &str, ctx: &Context) -> Result<Value, String> {
    if super::native_views::owns(id) {
        return super::native_views::resolve(id, &ctx.directory, &ctx.paths);
    }
    file_ops::resolve(id, ctx)
}

#[cfg(test)]
pub(super) fn execute(id: &str, item_id: &str, ctx: &Context) -> Result<Value, String> {
    let offered = resolve(id, ctx)?;
    if !offered["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["id"] == item_id && item["disabled"] != true)
    }) {
        return Err("menu action is no longer available".into());
    }
    if super::native_views::owns(id) {
        return super::native_views::execute(id, item_id, &ctx.directory, &ctx.paths);
    }
    file_ops::execute(id, item_id, ctx)
}

pub(super) fn action_result(item_id: &str, ctx: &Context) -> MenuExecuteResult {
    let paths = if matches!(item_id, "remote.keepLocal" | "remote.keepRemote") {
        ctx.entries
            .iter()
            .filter(|entry| entry.sync.as_deref() == Some("conflict"))
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>()
    } else {
        ctx.paths.clone()
    };
    if item_id.starts_with("remote.") {
        MenuExecuteResult::remote_action(
            item_id,
            paths,
            ctx.paths.first().cloned().unwrap_or_default(),
            ctx.directory.clone(),
        )
    } else {
        MenuExecuteResult::core_action(
            item_id,
            paths,
            ctx.paths.first().cloned().unwrap_or_default(),
            ctx.directory.clone(),
        )
    }
}

#[cfg(test)]
mod location_tests {
    use super::*;

    #[test]
    fn host_only_actions_require_trusted_direct_local_storage() {
        assert!(plain_local("/tmp/example"));
        assert!(!plain_local("recent://"));
        assert!(!plain_local("applications://"));
        assert!(!plain_local("fiex://mount/unregistered/a.txt"));
        assert!(!plain_local("unregistered://somewhere"));
    }
}
