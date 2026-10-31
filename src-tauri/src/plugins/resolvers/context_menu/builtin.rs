//! Builtin menu policy. The frontend receives data, never provider closures.
use super::file_ops::{available, BuiltinResolver, Selection};
use crate::plugins::events::Resolver;
use crate::plugins::resource::file_capabilities::FileCapabilities;
use serde_json::{json, Value};

pub(super) struct Context {
    pub directory: String,
    pub paths: Vec<String>,
    pub entries: Vec<crate::core::files::Entry>,
    pub read_only: bool,
    pub in_trash: bool,
    all_local: bool,
    capabilities: FileCapabilities,
}

/// Host-only actions require a direct local FS resource. A location type
/// of "fs" alone is not enough: raw USBFS is also FS but must not be
/// accidentally passed to local operating-system commands.
fn plain_local(path: &str) -> bool {
    let (view, storage) = crate::plugins::match_rules::locations_for(path);
    matches!((view, storage),
        (Some(view), Some(storage))
            if view == storage && storage["providerId"] == "core:local-fs")
}

fn trash_view(path: &str) -> bool {
    crate::plugins::match_rules::locations_for(path)
        .0
        .is_some_and(|view| view["providerId"] == "core:trash")
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
        let read_only = entry(directory)?.readonly;
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
        let in_trash = trash_view(directory);
        Ok(Self {
            directory: directory.into(),
            paths: paths.to_vec(),
            entries,
            read_only,
            in_trash,
            all_local,
            capabilities,
        })
    }

    pub(super) fn selection(&self) -> Selection<'_> {
        Selection {
            paths: &self.paths,
            entries: &self.entries,
            read_only: self.read_only,
            in_trash: self.in_trash,
            all_local: self.all_local,
        }
    }

    /// Resolvers whose capability exists for this selection.
    pub(super) fn resolvers(&self) -> Vec<&'static dyn BuiltinResolver> {
        let selection = self.selection();
        available(&selection, &self.capabilities)
    }

    /// Tests build menus without touching the mount registry.
    #[cfg(test)]
    pub(super) fn for_test(
        directory: &str,
        paths: Vec<String>,
        entries: Vec<crate::core::files::Entry>,
        read_only: bool,
        in_trash: bool,
    ) -> Self {
        let capabilities =
            crate::plugins::resource::file_capabilities::for_selection(&paths, &entries);
        Self::for_test_caps(directory, paths, entries, read_only, in_trash, capabilities)
    }

    /// Tests pin the capability snapshot to exercise menu gating directly.
    #[cfg(test)]
    pub(super) fn for_test_caps(
        directory: &str,
        paths: Vec<String>,
        entries: Vec<crate::core::files::Entry>,
        read_only: bool,
        in_trash: bool,
        capabilities: FileCapabilities,
    ) -> Self {
        let all_local = plain_local(directory) && paths.iter().all(|path| plain_local(path));
        Self {
            directory: directory.into(),
            paths,
            entries,
            read_only,
            in_trash,
            all_local,
            capabilities,
        }
    }
}

/// Only groups this selection can actually use become menu providers.
fn retain_unclaimed(ctx: &Context, items: Vec<Value>) -> Vec<Value> {
    let claimed = super::native_views::claimed_actions(&ctx.directory, &ctx.paths);
    items
        .into_iter()
        .filter(|item| !claimed.iter().any(|id| item["id"].as_str() == Some(*id)))
        .collect()
}

/// Native and plugin contributions participate in the same EventRouter.
/// A native view claims only its OS-specific action identifiers, not the
/// entire menu. Generic providers still contribute independent actions.
pub(super) fn providers(ctx: &Context) -> Vec<Resolver> {
    let mut candidates = super::native_views::providers(&ctx.directory, &ctx.paths);
    let selection = ctx.selection();
    candidates.extend(
        ctx.resolvers()
            .into_iter()
            .filter(|resolver| {
                !retain_unclaimed(ctx, resolver.items(&selection, &ctx.capabilities)).is_empty()
            })
            .map(|resolver| Resolver::builtin(resolver.id(), resolver.priority())),
    );
    candidates
}

pub(super) fn resolve(id: &str, ctx: &Context) -> Result<Value, String> {
    if super::native_views::owns(id) {
        return super::native_views::resolve(id, &ctx.directory, &ctx.paths);
    }
    let resolver = ctx
        .resolvers()
        .into_iter()
        .find(|resolver| resolver.id() == id)
        .ok_or("unknown or unavailable builtin menu provider")?;
    let selection = ctx.selection();
    Ok(json!({"items": retain_unclaimed(
        ctx,
        resolver.items(&selection, &ctx.capabilities),
    )}))
}

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
        let action = super::native_views::execute(id, item_id, &ctx.directory, &ctx.paths)?;
        return Ok(json!({
            "kind":"coreAction", "command":action.command, "paths":action.paths,
            "path":ctx.paths.first().cloned().unwrap_or_default(), "directory":ctx.directory
        }));
    }
    let paths = if matches!(item_id, "remote.keepLocal" | "remote.keepRemote") {
        ctx.entries
            .iter()
            .filter(|entry| entry.sync.as_deref() == Some("conflict"))
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>()
    } else {
        ctx.paths.clone()
    };
    Ok(
        json!({"kind":if item_id.starts_with("remote.") { "remoteAction" } else { "coreAction" },
        "command":item_id,"paths":paths,"path":ctx.paths.first().cloned().unwrap_or_default(),"directory":ctx.directory}),
    )
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
        assert!(trash_view("recyclebin://"));
        assert!(!trash_view("/tmp/example"));
    }
}
