//! Builtin menu policy. The frontend receives data, never provider closures.
use super::file_ops::{self, Selection};
use crate::plugins::events::Resolver;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::MenuExecuteResult;
use fiex_resource::FileCapabilities;
#[cfg(test)]
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct Context {
    pub directory: String,
    pub paths: Vec<String>,
    pub entries: Vec<crate::core::files::Entry>,
    pub resources: Arc<Vec<ResourceSnapshot>>,
    pub match_targets: Vec<fiex_provider_contracts::MatchResource>,
    pub directory_resource: Option<ResourceSnapshot>,
    pub directory_target: Option<fiex_provider_contracts::MatchResource>,
    all_local: bool,
    capabilities: FileCapabilities,
    pub(super) preflight: MenuPreflightAvailability,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MenuPreflightAvailability {
    pub new_file: bool,
    pub new_folder: bool,
    pub rename: bool,
    pub batch_rename: bool,
    pub trash: bool,
    pub remove: bool,
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
    pub fn load_for_menu(
        directory: &str,
        paths: &[String],
        resolve_preflight: bool,
    ) -> Result<Self, String> {
        Self::load_internal(directory, paths, resolve_preflight)
    }

    fn load_internal(
        directory: &str,
        paths: &[String],
        resolve_preflight: bool,
    ) -> Result<Self, String> {
        // Capture the displayed directory once. This is a presentation hint;
        // execution still reauthorizes every selected resource.
        let snapshot = crate::core::fs::directory_cache::read_snapshot(directory, true, &[])?;
        let cached = snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .listing
                    .entries
                    .iter()
                    .map(|entry| (entry.path.as_str(), entry))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default();
        let local_entry = |path: &str| {
            crate::core::fs::trusted(path, "core:context-menu")?
                .desktop_entry()
                .map_err(String::from)
        };
        let entries = paths
            .iter()
            .map(|path| {
                if matches!(
                    crate::core::fs::provider_registry::owner(path),
                    Ok(crate::core::fs::provider_registry::ResourceOwner::LocalFs)
                ) {
                    return local_entry(path);
                }
                cached
                    .get(path.as_str())
                    .map(|entry| (**entry).clone())
                    .ok_or_else(|| {
                        "selected VFS item is not in the current directory snapshot".into()
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let directory_entry = if matches!(
            crate::core::fs::provider_registry::owner(directory),
            Ok(crate::core::fs::provider_registry::ResourceOwner::LocalFs)
        ) {
            Some(local_entry(directory)?)
        } else {
            snapshot.as_ref().map(|snapshot| {
                let uri_name = crate::plugins::mounts::parse(directory)
                    .map(|uri| uri.relative)
                    .unwrap_or_else(|_| {
                        directory
                            .split_once("://")
                            .map(|(_, relative)| relative.to_owned())
                            .unwrap_or_default()
                    })
                    .trim_end_matches('/')
                    .rsplit('/')
                    .find(|name| !name.is_empty())
                    .or_else(|| directory.split_once("://").map(|(scheme, _)| scheme))
                    .unwrap_or("Connection")
                    .to_owned();
                crate::core::files::Entry {
                    path: directory.into(),
                    name: uri_name,
                    is_dir: true,
                    allowed_ops: snapshot.listing.allowed_ops.clone(),
                    ..Default::default()
                }
            })
        };
        let capabilities = if paths.is_empty() {
            directory_entry.as_ref().map_or_else(
                || crate::plugins::resource::file_capabilities::for_path(directory),
                |entry| crate::plugins::resource::file_capabilities::for_entry(directory, entry),
            )
        } else {
            crate::plugins::resource::file_capabilities::for_selection(paths, &entries)
        };
        let resources = Arc::new(
            entries
                .iter()
                .map(|entry| {
                    crate::plugins::resource::resource_context::from_entry(&entry.path, entry)
                })
                .collect::<Vec<_>>(),
        );
        let match_targets = crate::plugins::events::matching::match_resources(resources.as_slice());
        let directory_resource = directory_entry
            .as_ref()
            .map(|entry| crate::plugins::resource::resource_context::from_entry(directory, entry));
        let directory_target = crate::plugins::match_rules::directory_match_resource(directory);
        let all_local = plain_local(directory) && paths.iter().all(|path| plain_local(path));
        let mut context = Self {
            directory: directory.into(),
            paths: paths.to_vec(),
            entries,
            resources,
            match_targets,
            directory_resource,
            directory_target,
            all_local,
            capabilities,
            preflight: MenuPreflightAvailability::default(),
        };
        if resolve_preflight {
            context.preflight = file_ops::preflight_availability(&context);
        }
        Ok(context)
    }

    pub(super) fn selection(&self) -> Selection<'_> {
        Selection {
            paths: &self.paths,
            entries: &self.entries,
            all_local: self.all_local,
            preflight: self.preflight,
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
        let mut resources = entries
            .iter()
            .map(|entry| crate::plugins::resource::resource_context::from_entry(&entry.path, entry))
            .collect::<Vec<_>>();
        for resource in &mut resources {
            resource.operations = Some(capabilities);
        }
        let resources = Arc::new(resources);
        let mut match_targets =
            crate::plugins::events::matching::match_resources(resources.as_slice());
        let allowed_operations = capabilities
            .into_allowed_ops()
            .iter()
            .filter_map(|operation| fiex_provider_contracts::FsOperation::parse(operation))
            .collect::<Vec<_>>();
        for target in &mut match_targets {
            target.capabilities.operations = allowed_operations.clone();
        }
        let mut directory_resource =
            Some(crate::plugins::resource::resource_context::from_metadata(
                directory,
                crate::plugins::contracts::ResourceMetadata {
                    name: directory.into(),
                    kind: "directory".into(),
                    ..Default::default()
                },
            ));
        if let Some(resource) = directory_resource.as_mut() {
            resource.operations = Some(capabilities);
        }
        let mut directory_target = crate::plugins::match_rules::directory_match_resource(directory);
        if let Some(target) = directory_target.as_mut() {
            target.capabilities.operations = allowed_operations;
        }
        let all_local = plain_local(directory) && paths.iter().all(|path| plain_local(path));
        Self {
            directory: directory.into(),
            paths,
            entries,
            resources,
            match_targets,
            directory_resource,
            directory_target,
            all_local,
            capabilities,
            preflight: MenuPreflightAvailability {
                new_file: true,
                new_folder: true,
                rename: true,
                batch_rename: true,
                trash: true,
                remove: true,
            },
        }
    }
}

/// All Core providers participate in one in-process resolver pass.
pub(super) fn providers(ctx: &Context) -> Vec<Resolver> {
    file_ops::providers(ctx)
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
