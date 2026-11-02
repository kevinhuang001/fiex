//! Builtin menu policy. The frontend receives data, never provider closures.
use super::file_ops::{self, Selection};
use crate::plugins::events::Resolver;
use crate::plugins::resource::file_capabilities::FileCapabilities;
use fiex_provider_contracts::{MenuExecuteResult, MenuItem};
#[cfg(test)]
use serde_json::Value;

#[derive(Clone)]
pub(super) struct Context {
    pub directory: String,
    pub directory_entry: Option<crate::core::files::Entry>,
    pub paths: Vec<String>,
    pub entries: Vec<crate::core::files::Entry>,
    all_local: bool,
    capabilities: FileCapabilities,
    native_view_providers: Vec<Resolver>,
    claimed_actions: Vec<String>,
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
        // Provider-backed UI hints come from the current authorized directory
        // snapshot, so the first Core menu never performs remote stat IPC.
        // Every actual action still reauthorizes through ResourceBroker.
        fn entry(path: &str) -> Result<crate::core::files::Entry, String> {
            if crate::core::fs::provider_registry::is_plugin_mount(path) {
                let parent = crate::core::fs::provider_registry::parent_path(path)?;
                return crate::core::fs::directory_cache::cached_entry(&parent, path)?.ok_or_else(
                    || "selected VFS item is not in the current directory snapshot".into(),
                );
            }
            crate::core::fs::trusted(path, "core:context-menu")?
                .desktop_entry()
                .map_err(String::from)
        }

        fn directory_entry(path: &str) -> Result<crate::core::files::Entry, String> {
            if !crate::core::fs::provider_registry::is_plugin_mount(path) {
                return entry(path);
            }
            if let Ok(parent) = crate::core::fs::provider_registry::parent_path(path) {
                if let Some(entry) = crate::core::fs::directory_cache::cached_entry(&parent, path)?
                {
                    return Ok(entry);
                }
            }
            let snapshot = crate::core::fs::directory_cache::read_snapshot(path, true, &[])?
                .ok_or_else(|| {
                    "current VFS directory is not in the directory snapshot".to_owned()
                })?;
            let uri = crate::plugins::mounts::parse(path)?;
            let name = uri
                .relative
                .rsplit('/')
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or("Connection")
                .to_owned();
            Ok(crate::core::files::Entry {
                path: path.into(),
                name,
                is_dir: true,
                allowed_ops: snapshot.listing.allowed_ops,
                ..Default::default()
            })
        }

        let entries = paths
            .iter()
            .map(|path| entry(path))
            .collect::<Result<Vec<_>, _>>()?;
        let directory_entry = directory_entry(directory)?;
        let capabilities = if paths.is_empty() {
            crate::plugins::resource::file_capabilities::for_path(directory)
        } else {
            crate::plugins::resource::file_capabilities::for_selection(paths, &entries)
        };
        let all_local = plain_local(directory) && paths.iter().all(|path| plain_local(path));
        let (native_view_providers, claimed_actions) =
            super::native_views::resolve_snapshot(directory, paths);
        Ok(Self {
            directory: directory.into(),
            directory_entry: Some(directory_entry),
            paths: paths.to_vec(),
            entries,
            all_local,
            capabilities,
            native_view_providers,
            claimed_actions,
        })
    }

    pub(super) fn selection(&self) -> Selection<'_> {
        Selection {
            paths: &self.paths,
            entries: &self.entries,
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
    ) -> Self {
        let capabilities =
            crate::plugins::resource::file_capabilities::for_selection(&paths, &entries);
        Self::for_test_caps(directory, paths, entries, capabilities)
    }

    /// Tests pin the capability snapshot to exercise menu gating directly.
    #[cfg(test)]
    pub(super) fn for_test_caps(
        directory: &str,
        paths: Vec<String>,
        entries: Vec<crate::core::files::Entry>,
        capabilities: FileCapabilities,
    ) -> Self {
        let all_local = plain_local(directory) && paths.iter().all(|path| plain_local(path));
        let (native_view_providers, claimed_actions) =
            super::native_views::resolve_snapshot(directory, &paths);
        Self {
            directory: directory.into(),
            directory_entry: None,
            paths,
            entries,
            all_local,
            capabilities,
            native_view_providers,
            claimed_actions,
        }
    }
}

/// Only groups this selection can actually use become menu providers.
pub(super) fn retain_unclaimed(ctx: &Context, items: Vec<MenuItem>) -> Vec<MenuItem> {
    items
        .into_iter()
        .filter(|item| !ctx.claimed_actions.contains(&item.id))
        .collect()
}

/// Native and plugin contributions participate in the same EventRouter.
/// A native view claims only its OS-specific action identifiers, not the
/// entire menu. Generic providers still contribute independent actions.
pub(super) fn providers(ctx: &Context) -> Vec<Resolver> {
    let mut candidates = ctx.native_view_providers.clone();
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
