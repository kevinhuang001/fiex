//! Builtin menu policy. The frontend receives data, never provider closures.
use super::file_ops::{groups, BuiltinResolver, Selection};
use crate::plugin_events::Provider;
use crate::plugin_file_capabilities::FileCapabilities;
use serde_json::{json, Value};

pub(super) struct Context {
    pub directory: String,
    pub paths: Vec<String>,
    pub entries: Vec<crate::files::Entry>,
    pub read_only: bool,
    pub in_trash: bool,
    all_local: bool,
    capabilities: FileCapabilities,
}

impl Context {
    pub fn load(directory: &str, paths: &[String]) -> Result<Self, String> {
        fn entry(path: &str) -> Result<crate::files::Entry, String> {
            if crate::remote::is_remote(path) {
                crate::remote::stat(path)
            } else if crate::mounts::handles(path) {
                crate::plugin_vfs::stat(path)
            } else {
                crate::files::entry(std::path::Path::new(path))
            }
        }
        let entries = paths
            .iter()
            .map(|path| entry(path))
            .collect::<Result<Vec<_>, _>>()?;
        // `recent://` is a Core-owned view, not a directory on disk.
        let read_only = if directory == "recent://" {
            true
        } else {
            entry(directory)?.readonly
        };
        let capabilities = crate::plugin_file_capabilities::for_selection(paths, &entries);
        let all_local = !crate::mounts::handles(directory)
            && paths.iter().all(|path| !crate::mounts::handles(path));
        let in_trash = !crate::mounts::handles(directory)
            && crate::volumes::trash_path()
                .is_some_and(|trash| std::path::Path::new(directory).starts_with(trash));
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
        entries: Vec<crate::files::Entry>,
        read_only: bool,
        in_trash: bool,
    ) -> Self {
        let capabilities = crate::plugin_file_capabilities::for_selection(&paths, &entries);
        Self::for_test_caps(directory, paths, entries, read_only, in_trash, capabilities)
    }

    /// Tests pin the capability snapshot to exercise menu gating directly.
    #[cfg(test)]
    pub(super) fn for_test_caps(
        directory: &str,
        paths: Vec<String>,
        entries: Vec<crate::files::Entry>,
        read_only: bool,
        in_trash: bool,
        capabilities: FileCapabilities,
    ) -> Self {
        let all_local = !crate::mounts::handles(directory)
            && paths.iter().all(|path| !crate::mounts::handles(path));
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
pub(super) fn providers(ctx: &Context) -> Vec<Provider> {
    ctx.resolvers()
        .into_iter()
        .map(|resolver| Provider::builtin(resolver.id(), resolver.priority()))
        .collect()
}

pub(super) fn resolve(id: &str, ctx: &Context) -> Result<Value, String> {
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
        "command":item_id,"paths":paths,"path":ctx.paths[0],"directory":ctx.directory}),
    )
}
