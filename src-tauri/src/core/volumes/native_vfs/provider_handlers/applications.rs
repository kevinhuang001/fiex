use super::shared::vfs_identity;
use crate::core::volumes::native_vfs::{
    APPLICATIONS_URI, NativeVfsBehavior, NativeVfsIdentity, application_provider, applications,
};

fn applications_is_tree_path(path: &str) -> bool {
    path == APPLICATIONS_URI
        || applications::is_folder(path)
        || applications::is_app(path)
        || applications::is_user(path)
}

fn applications_resource_kind(path: &str) -> &'static str {
    if applications::is_folder(path) || path.ends_with('/') {
        "directory"
    } else {
        "file"
    }
}

fn applications_icon(path: &str) -> Option<String> {
    if applications::is_user(path) {
        return None;
    }
    let entry = applications::system_entry(path).ok()?;
    applications::cached_icon(entry.link_target.as_deref()?)
}

fn applications_default_view(_path: &str) -> Option<&'static str> {
    Some("icons")
}

fn applications_child_path(
    parent: &str,
    name: &str,
    kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<String, String> {
    match kind {
        crate::core::fs::provider_registry::ChildKind::Folder => {
            applications::reserve_child(parent, name, true)
        }
        crate::core::fs::provider_registry::ChildKind::Application => {
            applications::reserve_child(parent, name, false)
        }
        crate::core::fs::provider_registry::ChildKind::File => {
            Err("Applications does not support creating ordinary files".into())
        }
    }
}

pub(crate) struct ApplicationsOperations;

impl NativeVfsBehavior for ApplicationsOperations {
    #[cfg(feature = "desktop")]
    fn open(&self, path: &str) -> Option<Result<(), String>> {
        Some(applications::open(path))
    }

    fn validate_child(&self, parent: &str, child: &str) -> Option<Result<(), String>> {
        Some(applications::validate_child(parent, child))
    }

    fn child_path(
        &self,
        parent: &str,
        name: &str,
        kind: crate::core::fs::provider_registry::ChildKind,
    ) -> Option<Result<String, String>> {
        Some(applications_child_path(parent, name, kind))
    }

    fn parent_path(&self, path: &str) -> Option<Result<String, String>> {
        Some(applications::parent_uri(path))
    }

    fn same_mount_move(
        &self,
        sources: &[crate::core::fs::ResourceBroker],
        destination: &crate::core::fs::ResourceBroker,
    ) -> Option<Result<Vec<String>, String>> {
        Some(application_provider::move_within_mount(
            sources,
            destination,
        ))
    }

    fn backing_path(&self, path: &str) -> Option<std::path::PathBuf> {
        application_provider::backing_path(path)
    }

    fn after_rename(&self, path: &str) {
        applications::cancel_reservation(path);
    }

    fn is_tree_path(&self, path: &str) -> bool {
        applications_is_tree_path(path)
    }

    fn resource_kind(&self, path: &str) -> Option<&'static str> {
        Some(applications_resource_kind(path))
    }

    fn icon(&self, path: &str) -> Option<String> {
        applications_icon(path)
    }

    fn default_view(&self, path: &str) -> Option<&'static str> {
        applications_default_view(path)
    }

    fn provider_identity(
        &self,
        path: &str,
        provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        vfs_identity(path, provider_id)
    }
}
