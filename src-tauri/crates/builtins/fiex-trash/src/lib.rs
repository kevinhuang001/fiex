//! Trash VFS, OS operations, and its view-specific actions.
mod journal;
mod menu;
mod operations;
mod restore_fs;
mod vfs;

use fiex_provider_contracts::{
    ContextMenuProvider, ProviderError, ProviderInvocation, VfsProvider,
};
use fiex_storage::CoreOwnerKey;
use std::sync::Arc;
use std::sync::OnceLock;

mod path_trash;
mod recycle_bin;

pub use operations::{
    empty_trash, move_to_trash, restore_from_trash, restore_needs_folder, trash_path,
};
pub use path_trash::TrashDirectory;
pub use recycle_bin::{RecycleBin, RECYCLE_BIN_URI};

pub const TRASH_URI: &str = "trash://";
pub const PROVIDER_ID: &str = "core:trash";
const URI_SCHEMES: &[&str] = &["trash", "recyclebin"];

pub fn owner_key() -> CoreOwnerKey {
    CoreOwnerKey::new("core")
}

pub fn uri_schemes() -> &'static [&'static str] {
    URI_SCHEMES
}

pub fn is_recycle_bin_uri(uri: &str) -> bool {
    recycle_bin::is_virtual(uri)
}

pub fn system_recycle_bin() -> &'static RecycleBin {
    static BACKEND: OnceLock<RecycleBin> = OnceLock::new();
    BACKEND.get_or_init(RecycleBin::system)
}

pub fn system_trash_directory() -> &'static TrashDirectory {
    static BACKEND: OnceLock<TrashDirectory> = OnceLock::new();
    BACKEND.get_or_init(TrashDirectory::system)
}

/// Core supplies invocation context, search policy, and localized strings.
/// Trash owns backend routing and all mutations of Trash contents.
pub trait TrashModuleServices: Send + Sync {
    fn path(&self, ctx: &dyn ProviderInvocation) -> Result<String, ProviderError>;
    fn search(
        &self,
        root: &str,
        request: &fiex_provider_contracts::FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsListing, ProviderError>;
    fn selection(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<(String, Vec<String>), ProviderError>;
    fn translate(&self, text: &'static str) -> String;
    fn map_error(&self, error: String) -> String;
}

pub struct TrashModule {
    services: Arc<dyn TrashModuleServices>,
}

impl TrashModule {
    pub fn new(services: Arc<dyn TrashModuleServices>) -> Self {
        Self { services }
    }

    pub fn vfs(&self) -> Arc<dyn VfsProvider> {
        Arc::new(vfs::TrashVfs::new(self.services.clone()))
    }

    pub fn restore_menu(&self) -> Arc<dyn ContextMenuProvider> {
        Arc::new(menu::RestoreMenu::new(self.services.clone()))
    }

    pub fn empty_menu(&self) -> Arc<dyn ContextMenuProvider> {
        Arc::new(menu::EmptyTrashMenu::new(self.services.clone()))
    }
}
