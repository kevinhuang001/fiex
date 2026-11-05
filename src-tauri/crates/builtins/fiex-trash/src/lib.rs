//! Trash VFS and its view-specific restore and empty actions.
use fiex_provider_contracts::{
    ContextMenuProvider, FsEntry, FsListing, FsOperation, FsRequest, MenuExecuteResult, MenuItem,
    MenuPlacement, ProviderError, ProviderInput, ProviderInvocation, SearchCapabilities,
    VfsProvider,
};
use fiex_storage::CoreOwnerKey;
use std::sync::Arc;
use std::sync::OnceLock;

mod path_trash;
mod recycle_bin;

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

/// Core supplies only trusted URI resolution, platform operations, and the
/// shared storage projection. Trash owns the provider and menu semantics.
pub trait TrashModuleServices: Send + Sync {
    fn path(&self, ctx: &dyn ProviderInvocation) -> Result<String, ProviderError>;
    fn capabilities(&self, uri: &str) -> Vec<FsOperation>;
    fn stat(&self, uri: &str, operations: &[FsOperation]) -> Result<FsEntry, ProviderError>;
    fn list(&self, uri: &str) -> Result<FsListing, ProviderError>;
    fn purge(&self, uri: &str) -> Result<(), ProviderError>;
    fn search(&self, root: &str, request: &FsRequest<'_>) -> Result<FsListing, ProviderError>;
    fn selection(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<(String, Vec<String>), ProviderError>;
    fn translate(&self, text: &'static str) -> String;
}

pub struct TrashModule {
    services: Arc<dyn TrashModuleServices>,
}

impl TrashModule {
    pub fn new(services: Arc<dyn TrashModuleServices>) -> Self {
        Self { services }
    }

    pub fn vfs(&self) -> Arc<dyn VfsProvider> {
        Arc::new(TrashVfs::new(self.services.clone()))
    }

    pub fn restore_menu(&self) -> Arc<dyn ContextMenuProvider> {
        Arc::new(RestoreMenu::new(self.services.clone()))
    }

    pub fn empty_menu(&self) -> Arc<dyn ContextMenuProvider> {
        Arc::new(EmptyTrashMenu::new(self.services.clone()))
    }
}

fn is_root(uri: &str) -> bool {
    uri == TRASH_URI || uri == RECYCLE_BIN_URI
}

fn offered_item(ctx: &dyn ProviderInvocation) -> Result<&str, ProviderError> {
    match ctx.input()? {
        ProviderInput::ContextMenuExecute(request) => request
            .item_id
            .as_deref()
            .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId")),
        _ => Err(ProviderError::invalid_params(
            "context menu execution input is required",
        )),
    }
}

fn action_item(id: &str, title: String, icon: &str, danger: bool) -> MenuItem {
    let mut item = MenuItem::action(id, title);
    item.icon = Some(icon.into());
    item.danger = danger;
    if danger {
        item.placement = Some(MenuPlacement::Tail);
    }
    item
}

fn core_action(command: &str, paths: Vec<String>, directory: &str) -> MenuExecuteResult {
    let path = paths.first().cloned().unwrap_or_default();
    MenuExecuteResult::core_action(command, paths, path, directory)
}

struct RestoreMenu {
    services: Arc<dyn TrashModuleServices>,
}

impl RestoreMenu {
    fn new(services: Arc<dyn TrashModuleServices>) -> Self {
        Self { services }
    }
}

impl ContextMenuProvider for RestoreMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (_, paths) = self.services.selection(ctx)?;
        if paths.len() != 1 {
            return Ok(Vec::new());
        }
        Ok(vec![action_item(
            "restore",
            self.services.translate("放回原处"),
            "restore",
            false,
        )])
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = self.services.selection(ctx)?;
        if offered_item(ctx)? != "restore" || paths.len() != 1 {
            return Err(ProviderError::invalid_params(
                "restore requires one Trash item",
            ));
        }
        Ok(core_action("restore", paths, &directory))
    }
}

struct EmptyTrashMenu {
    services: Arc<dyn TrashModuleServices>,
}

impl EmptyTrashMenu {
    fn new(services: Arc<dyn TrashModuleServices>) -> Self {
        Self { services }
    }
}

impl ContextMenuProvider for EmptyTrashMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = self.services.selection(ctx)?;
        if !paths.is_empty() || !is_root(&directory) {
            return Ok(Vec::new());
        }
        let title = if cfg!(windows) {
            "清空回收站"
        } else {
            "清空废纸篓"
        };
        Ok(vec![action_item(
            "emptyTrash",
            self.services.translate(title),
            "trash",
            true,
        )])
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = self.services.selection(ctx)?;
        if offered_item(ctx)? != "emptyTrash" || !paths.is_empty() || !is_root(&directory) {
            return Err(ProviderError::invalid_params(
                "Empty Trash requires its root",
            ));
        }
        Ok(core_action("emptyTrash", paths, &directory))
    }
}

struct TrashVfs {
    services: Arc<dyn TrashModuleServices>,
}

impl TrashVfs {
    fn new(services: Arc<dyn TrashModuleServices>) -> Self {
        Self { services }
    }

    fn operations(&self, uri: &str) -> Vec<FsOperation> {
        if is_root(uri) {
            vec![FsOperation::List, FsOperation::Stat, FsOperation::Search]
        } else {
            self.services.capabilities(uri)
        }
    }
}

impl VfsProvider for TrashVfs {
    fn operations(&self) -> &'static [FsOperation] {
        &[
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Search,
            FsOperation::Remove,
        ]
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        Ok(self.operations(&self.services.path(ctx)?))
    }

    fn authorize(
        &self,
        ctx: &dyn ProviderInvocation,
        operation: FsOperation,
        _create: bool,
    ) -> Result<(), ProviderError> {
        if matches!(
            operation,
            FsOperation::Write | FsOperation::Mkdir | FsOperation::Rename | FsOperation::Trash
        ) {
            return Err(ProviderError::invalid_params(
                "PermissionDenied: Trash only supports controlled permanent removal",
            ));
        }
        if self
            .operations(&self.services.path(ctx)?)
            .contains(&operation)
        {
            Ok(())
        } else {
            Err(ProviderError::method_not_found("vfs.operation"))
        }
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let uri = self.services.path(request.ctx())?;
        self.services.stat(&uri, &self.operations(&uri))
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        self.services.list(&self.services.path(request.ctx())?)
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        let uri = self.services.path(request.ctx())?;
        if request.node_id() != Some(uri.as_str()) {
            return Err(ProviderError::invalid_params(
                "nodeId does not match scoped Trash resource",
            ));
        }
        self.authorize(request.ctx(), FsOperation::Remove, false)?;
        if is_root(&uri) {
            return Err(ProviderError::invalid_params(
                "ScopeDenied: Trash root cannot be permanently removed",
            ));
        }
        self.services.purge(&uri)
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        SearchCapabilities {
            name: self
                .services
                .path(ctx)
                .is_ok_and(|uri| self.operations(&uri).contains(&FsOperation::Search)),
            content: false,
        }
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        let root = self.services.path(request.ctx())?;
        self.services.search(&root, request)
    }
}
