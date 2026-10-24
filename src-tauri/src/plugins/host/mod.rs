//! Manifest-driven installed-plugin discovery. Every plugin must be approved
//! by the user; no ID, publisher name or package location grants privileges.
mod catalog;
mod manifest;
mod panel;
mod permissions;
mod process;

pub use catalog::{catalog, exists, installed_manifests, themes, PluginInfo};
pub use manifest::{
    activation_requirements, declared_permissions, valid_id, CommandsPermission, FilePermission,
    FolderPermission, FolderReadPermission, HooksPermission, NetworkPermission, PluginAuthor,
    PluginManifest, PluginPermissions, PluginPublisher, PluginRuntime,
};
pub use panel::{panel_asset, panel_html};
pub use permissions::{
    activate, approve_install, declared_permissions_for, network_domains, permission_allowed,
    set_command, set_enabled, set_network_domain, set_permission,
};
pub use process::{global, run_command, PluginHost, PluginTaskInfo};
