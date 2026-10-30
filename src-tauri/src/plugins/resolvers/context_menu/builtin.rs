//! Builtin menu policy. The frontend receives data, never provider closures.
use super::file_ops::{groups, BuiltinResolver, Selection};
use crate::plugins::events::Resolver;
use crate::plugins::resource::file_capabilities::FileCapabilities;
use serde_json::{json, Value};

pub(super) struct Context {
    pub directory: String,
    pub paths: Vec<String>,
    pub entries: Vec<crate::core::files::Entry>,
    pub read_only: bool,
    pub in_trash: bool,
    virtual_provider: Option<&'static dyn crate::core::volumes::virtual_resolvers::VirtualView>,
    all_local: bool,
    capabilities: FileCapabilities,
}

impl Context {
    pub fn load(directory: &str, paths: &[String]) -> Result<Self, String> {
        fn entry(path: &str) -> Result<crate::core::files::Entry, String> {
            if crate::core::volumes::virtual_resolvers::opaque(path) {
                crate::core::volumes::virtual_resolvers::resource_entry(path)
            } else if let Some(route) = crate::usbfs::route(path) {
                crate::usbfs::entry(&route)
            } else if crate::remote::is_remote(path) {
                crate::remote::stat(path)
            } else if crate::plugins::mounts::handles(path) {
                crate::plugins::vfs::stat(path)
            } else {
                crate::core::files::entry(std::path::Path::new(path))
            }
        }
        let entries = paths
            .iter()
            .map(|path| entry(path))
            .collect::<Result<Vec<_>, _>>()?;
        let provider = crate::core::volumes::virtual_resolvers::view_for(directory);
        // Recent is a projection of original resources and never owns the
        // menu. Applications selections (including user-store entries and
        // folders) use the Applications view; its blank canvas keeps the
        // capability-gated create menu.
        let virtual_provider = provider.filter(|p| {
            p.id() != "core:recent"
                && !(p.id() == "core:applications" && paths.is_empty())
        });
        if let Some(owner) = virtual_provider {
            if owner.id() == "core:trash"
                && paths.iter().any(|path| {
                    crate::core::volumes::virtual_resolvers::view_for(path)
                        .is_none_or(|p| p.id() != owner.id())
                })
            {
                return Err("selection outside VFS".into());
            }
        }
        let read_only = if let Some(provider) = provider {
            provider.readonly()
        } else {
            entry(directory)?.readonly
        };
        // A blank-canvas menu has no selection: capabilities come from the
        // directory itself so "new folder/file" follows the same grant.
        let mut capabilities = if paths.is_empty() {
            crate::plugins::resource::file_capabilities::for_path(directory)
        } else {
            crate::plugins::resource::file_capabilities::for_selection(paths, &entries)
        };
        if read_only || (directory == "recent://" && paths.is_empty()) {
            capabilities = capabilities.intersect(
                crate::plugins::resource::file_capabilities::FileCapabilities::READ_ONLY,
            );
        }
        let all_local = crate::usbfs::route(directory).is_none()
            && !crate::core::volumes::virtual_resolvers::opaque(directory)
            && !crate::plugins::mounts::handles(directory)
            && paths.iter().all(|path| {
                !crate::plugins::mounts::handles(path)
                    && crate::usbfs::route(path).is_none()
                    && !crate::core::volumes::virtual_resolvers::opaque(path)
            });
        let in_trash = !crate::plugins::mounts::handles(directory)
            && crate::core::volumes::trash_path()
                .is_some_and(|trash| std::path::Path::new(directory).starts_with(trash));
        Ok(Self {
            directory: directory.into(),
            paths: paths.to_vec(),
            entries,
            read_only,
            in_trash,
            virtual_provider,
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
        groups()
            .iter()
            .copied()
            .filter(|resolver| resolver.available(&selection, &self.capabilities))
            .collect()
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
        let all_local = crate::usbfs::route(directory).is_none()
            && !crate::core::volumes::virtual_resolvers::opaque(directory)
            && !crate::plugins::mounts::handles(directory)
            && paths.iter().all(|path| {
                !crate::plugins::mounts::handles(path)
                    && crate::usbfs::route(path).is_none()
                    && !crate::core::volumes::virtual_resolvers::opaque(path)
            });
        Self {
            directory: directory.into(),
            paths,
            entries,
            read_only,
            in_trash,
            virtual_provider: None,
            all_local,
            capabilities,
        }
    }
}

/// Only groups this selection can actually use become menu providers.
pub(super) fn providers(ctx: &Context) -> Vec<Resolver> {
    if ctx.virtual_provider.is_some() {
        return vec![Resolver::builtin("builtin:vfs-menu", 100)];
    }
    ctx.resolvers()
        .into_iter()
        .map(|resolver| Resolver::builtin(resolver.id(), resolver.priority()))
        .collect()
}

pub(super) fn resolve(id: &str, ctx: &Context) -> Result<Value, String> {
    if id == "builtin:vfs-menu" {
        let provider = ctx.virtual_provider.ok_or("VFS owner unavailable")?;
        return Ok(crate::core::volumes::virtual_resolvers::menu_json(
            provider, &ctx.paths,
        ));
    }
    let resolver = ctx
        .resolvers()
        .into_iter()
        .find(|resolver| resolver.id() == id)
        .ok_or("unknown or unavailable builtin menu provider")?;
    let selection = ctx.selection();
    Ok(json!({"items": resolver.items(&selection, &ctx.capabilities)}))
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
    if id == "builtin:vfs-menu" {
        let provider = ctx.virtual_provider.ok_or("VFS owner unavailable")?;
        let action = provider.execute(item_id, &ctx.paths)?;
        // `path` is the frontend's single-entry handle for actions such as
        // "open"/"启动应用"; `paths` remains the full selection for batch ones.
        // Use the selection path, not the provider's transformed result: Recent
        // maps aliases to sources, but the frontend only knows the alias.
        return Ok(
            json!({"kind":"coreAction", "command":action.command, "paths":action.paths, "path":ctx.paths.first().cloned().unwrap_or_default(), "directory":ctx.directory}),
        );
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
