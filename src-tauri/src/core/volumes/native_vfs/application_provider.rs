//! Applications view implementation of the shared VFS provider contract.
use super::shared::*;
use super::*;
use fiex_provider_contracts::{
    ExportPreflight, ImportPreflight, MovePreflight, PreflightDecision, PreflightHandlers,
    PreflightPhase, ProviderError, ProviderInvocation, RemoveMode, RemovePreflight,
    RenamePreflight, VfsProvider,
};
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
};

fn application_child(parent: &str, name: &str) -> Result<(String, PathBuf), ProviderError> {
    crate::plugins::mounts::segment(name).map_err(provider_error)?;
    let parent_path = if parent == APPLICATIONS_URI {
        applications::stored_path(APPLICATIONS_URI)
    } else {
        applications::candidate_path(parent)
    }
    .map_err(provider_error)?;
    if !parent_path.is_dir() {
        return Err(ProviderError::invalid_params(
            "NotFound: application parent is not a directory",
        ));
    }
    let path = parent_path.join(name);
    let uri = applications::stored_uri(&path).map_err(provider_error)?;
    Ok((uri, path))
}

struct ApplicationStage(Arc<Mutex<Option<tempfile::NamedTempFile>>>);

impl Write for ApplicationStage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut temporary = self
            .0
            .lock()
            .map_err(|_| io::Error::other("write stream unavailable"))?;
        temporary
            .as_mut()
            .ok_or_else(|| io::Error::other("write stream closed"))?
            .write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut temporary = self
            .0
            .lock()
            .map_err(|_| io::Error::other("write stream unavailable"))?;
        temporary
            .as_mut()
            .ok_or_else(|| io::Error::other("write stream closed"))?
            .flush()
    }
}

fn application_operations(path: &str) -> Vec<FsOperation> {
    if path == APPLICATIONS_URI {
        vec![
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Search,
            FsOperation::Write,
            FsOperation::Mkdir,
        ]
    } else if applications::is_user(path) {
        applications::stored_entry(path)
            .map(|entry| {
                entry
                    .allowed_ops
                    .iter()
                    .filter_map(|operation| FsOperation::parse(operation))
                    .collect()
            })
            .unwrap_or_default()
    } else if applications::is_system_app(path) {
        vec![FsOperation::Stat]
    } else {
        Vec::new()
    }
}

pub(super) struct ApplicationView;

/// Resolve a user-store node to its backing path inside the Applications
/// provider. System-app identities are virtual and have no writable backing
/// path exposed through this helper.
pub(super) fn backing_path(uri: &str) -> Option<PathBuf> {
    applications::is_user(uri)
        .then(|| applications::candidate_path(uri).ok())
        .flatten()
}

fn application_preflight_handlers() -> PreflightHandlers {
    static HANDLERS: LazyLock<PreflightHandlers> = LazyLock::new(|| {
        let mut handlers = PreflightHandlers::new();
        handlers
            .on_create_file(|_, _, _| {
                Ok(PreflightDecision::deny(
                    "Applications does not support creating files",
                ))
            })
            .expect("create-file handler is unique");
        handlers
            .on_rename(check_application_rename)
            .expect("rename handler is unique");
        handlers
            .on_remove(check_application_remove)
            .expect("remove handler is unique");
        handlers
            .on_import(check_application_import)
            .expect("import handler is unique");
        handlers
            .on_export(check_application_export)
            .expect("export handler is unique");
        handlers
            .on_move(check_application_move)
            .expect("move handler is unique");
        handlers
    });
    HANDLERS.clone()
}

fn check_application_import(
    _ctx: &dyn ProviderInvocation,
    _phase: PreflightPhase,
    request: &ImportPreflight,
) -> Result<PreflightDecision, ProviderError> {
    if request.source.system == Some(true) || request.source.executable_shortcut != Some(true) {
        Ok(PreflightDecision::deny(
            "Applications accepts only verified applications or shortcuts",
        ))
    } else {
        Ok(PreflightDecision::allow())
    }
}

fn check_application_export(
    ctx: &dyn ProviderInvocation,
    _phase: PreflightPhase,
    request: &ExportPreflight,
) -> Result<PreflightDecision, ProviderError> {
    let source = broker(ctx)?;
    if request.node_id != source.path {
        return Err(ProviderError::invalid_params(
            "export node does not match the scoped resource",
        ));
    }
    let metadata = source.transfer_metadata().map_err(provider_error)?;
    if metadata.system != Some(true)
        && metadata.executable_shortcut == Some(true)
        && applications::is_user(&source.path)
    {
        Ok(PreflightDecision::allow())
    } else {
        Ok(PreflightDecision::deny(
            "Only user applications can leave the Applications view",
        ))
    }
}

fn check_application_rename(
    ctx: &dyn ProviderInvocation,
    _phase: PreflightPhase,
    request: &RenamePreflight,
) -> Result<PreflightDecision, ProviderError> {
    let source = broker(ctx)?;
    if request.node_id != source.path {
        return Err(ProviderError::invalid_params(
            "rename node does not match the scoped resource",
        ));
    }
    if !applications::is_user(&source.path) {
        return Ok(PreflightDecision::deny(
            "System applications cannot be renamed in Fiex",
        ));
    }
    if let Some(name) = request.new_name.as_deref() {
        crate::plugins::mounts::segment(name).map_err(ProviderError::invalid_params)?;
        crate::core::files::validate_name(name).map_err(ProviderError::invalid_params)?;
    }
    Ok(PreflightDecision::allow())
}

fn check_application_remove(
    ctx: &dyn ProviderInvocation,
    _phase: PreflightPhase,
    request: &RemovePreflight,
) -> Result<PreflightDecision, ProviderError> {
    let source = broker(ctx)?;
    if request.node_ids.len() != 1 || request.node_ids[0] != source.path {
        return Err(ProviderError::invalid_params(
            "remove IDs do not match the scoped resource",
        ));
    }
    if request.mode != RemoveMode::Permanent {
        return Ok(PreflightDecision::deny(
            "Applications items must be permanently deleted",
        ));
    }
    if !applications::is_user(&source.path) {
        return Ok(PreflightDecision::deny(
            "System applications cannot be deleted in Fiex",
        ));
    }
    if let Err(reason) = applications::validate_user_removal(&source.path) {
        return Ok(PreflightDecision::deny(reason));
    }
    Ok(PreflightDecision::allow())
}

fn check_application_move(
    ctx: &dyn ProviderInvocation,
    _phase: PreflightPhase,
    request: &MovePreflight,
) -> Result<PreflightDecision, ProviderError> {
    let source = broker(ctx)?;
    let target_parent = destination_broker(ctx)?;
    if request.node_id != source.path || request.target_parent_id != target_parent.path {
        return Err(ProviderError::invalid_params(
            "move IDs do not match the scoped resources",
        ));
    }
    if !applications::is_user(&source.path) && !applications::is_system_app(&source.path) {
        return Ok(PreflightDecision::deny(
            "Only Applications entries can be moved inside Applications",
        ));
    }
    if target_parent.path != APPLICATIONS_URI && !applications::is_user(&target_parent.path) {
        return Ok(PreflightDecision::deny(
            "Applications entries can only be moved within Applications",
        ));
    }
    Ok(PreflightDecision::allow())
}

impl VfsProvider for ApplicationView {
    fn preflight_handlers(&self) -> Result<PreflightHandlers, ProviderError> {
        Ok(application_preflight_handlers())
    }

    fn operations(&self) -> &'static [FsOperation] {
        &[
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Search,
            FsOperation::Read,
            FsOperation::Write,
            FsOperation::Mkdir,
            FsOperation::Rename,
            FsOperation::Remove,
        ]
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        Ok(application_operations(path(ctx)?))
    }

    fn authorize(
        &self,
        ctx: &dyn ProviderInvocation,
        operation: FsOperation,
        create: bool,
    ) -> Result<(), ProviderError> {
        let uri = path(ctx)?;
        if !create {
            return if application_operations(uri).contains(&operation) {
                Ok(())
            } else {
                Err(ProviderError::method_not_found(operation.methods()[0]))
            };
        }
        if !matches!(operation, FsOperation::Write | FsOperation::Mkdir) {
            return Err(ProviderError::method_not_found(operation.methods()[0]));
        }
        // Create authorization is requested against the new child URI. That
        // node does not exist yet, so capability checks must come from its
        // parent directory rather than `application_operations(uri)`.
        let parent = if uri == APPLICATIONS_URI {
            APPLICATIONS_URI.to_owned()
        } else if applications::is_user(uri) {
            let target = applications::candidate_path(uri).map_err(provider_error)?;
            let parent = target
                .parent()
                .ok_or_else(|| ProviderError::invalid_params("application parent is missing"))?;
            if parent == applications::stored_path(APPLICATIONS_URI).map_err(provider_error)? {
                APPLICATIONS_URI.to_owned()
            } else {
                applications::stored_uri(parent).map_err(provider_error)?
            }
        } else {
            return Err(ProviderError::method_not_found(operation.methods()[0]));
        };
        let parent_is_directory = parent == APPLICATIONS_URI
            || applications::stored_entry(&parent).is_ok_and(|entry| entry.is_dir);
        if parent_is_directory && application_operations(&parent).contains(&operation) {
            Ok(())
        } else {
            Err(ProviderError::method_not_found(operation.methods()[0]))
        }
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        SearchCapabilities {
            name: path(ctx)
                .is_ok_and(|path| application_operations(path).contains(&FsOperation::Search)),
            content: false,
        }
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let path = path(request.ctx())?;
        storage::applications_stat(path, &application_operations(path))
            .map(shared_entry)
            .map_err(provider_error)
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        storage::applications_list(path(request.ctx())?)
            .map(list_result)
            .map_err(provider_error)
    }

    fn read(&self, request: &FsRequest<'_>) -> Result<FsRead, ProviderError> {
        let uri = request.node_id().unwrap_or(path(request.ctx())?);
        if !applications::is_user(uri) {
            return Err(ProviderError::method_not_found("vfs.openRead"));
        }
        let file_path = applications::candidate_path(uri).map_err(provider_error)?;
        let file = std::fs::File::open(&file_path).map_err(provider_error)?;
        let size = file.metadata().map_err(provider_error)?.len();
        Ok(FsRead::new(file, Some(size)))
    }

    fn write(&self, request: &FsRequest<'_>) -> Result<FsWrite, ProviderError> {
        let (uri, target, create) = match (request.node_id(), request.parent_id(), request.name()) {
            (Some(uri), _, _) => (
                uri.to_owned(),
                applications::candidate_path(uri).map_err(provider_error)?,
                false,
            ),
            (None, Some(parent), Some(name)) => {
                let (uri, target) = application_child(parent, name)?;
                (uri, target, true)
            }
            _ => {
                return Err(ProviderError::invalid_params(
                    "write requires nodeId or parentId and name",
                ))
            }
        };
        if create {
            let parent = target
                .parent()
                .ok_or_else(|| ProviderError::invalid_params("application parent is missing"))?;
            if !parent.is_dir() || target.exists() {
                return Err(ProviderError::invalid_params(
                    "Conflict: application destination is unavailable",
                ));
            }
        } else if !target.is_file() {
            return Err(ProviderError::invalid_params(
                "Unsupported: application write requires a file",
            ));
        }
        let temporary = tempfile::NamedTempFile::new_in(
            target
                .parent()
                .ok_or_else(|| ProviderError::invalid_params("application parent is missing"))?,
        )
        .map_err(provider_error)?;
        let staged = Arc::new(Mutex::new(Some(temporary)));
        let writer = ApplicationStage(Arc::clone(&staged));
        let commit_stage = Arc::clone(&staged);
        Ok(FsWrite::with_commit(writer, move || {
            let temporary = commit_stage
                .lock()
                .map_err(|_| ProviderError::internal("write stream unavailable"))?
                .take()
                .ok_or_else(|| ProviderError::internal("write stream closed"))?;
            temporary
                .as_file()
                .sync_all()
                .map_err(|error| ProviderError::internal(error.to_string()))?;
            if create {
                temporary
                    .persist_noclobber(&target)
                    .map_err(|error| ProviderError::internal(error.error.to_string()))?;
            } else {
                temporary
                    .persist(&target)
                    .map_err(|error| ProviderError::internal(error.error.to_string()))?;
            }
            let entry = applications::stored_entry(&uri).map_err(provider_error)?;
            Ok(shared_entry(entry))
        }))
    }

    fn mkdir(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let parent = request
            .parent_id()
            .ok_or_else(|| ProviderError::invalid_params("parentId is required"))?;
        let name = request
            .name()
            .ok_or_else(|| ProviderError::invalid_params("name is required"))?;
        let uri = applications::create_stored(parent, name, true).map_err(provider_error)?;
        applications::stored_entry(&uri)
            .map(shared_entry)
            .map_err(provider_error)
    }

    fn rename(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let uri = request
            .node_id()
            .ok_or_else(|| ProviderError::invalid_params("nodeId is required"))?;
        let parent = request
            .parent_id()
            .ok_or_else(|| ProviderError::invalid_params("parentId is required"))?;
        let name = request
            .name()
            .ok_or_else(|| ProviderError::invalid_params("name is required"))?;
        let expected_parent = applications::stored_path(uri)
            .map_err(provider_error)?
            .parent()
            .ok_or_else(|| ProviderError::invalid_params("application item has no parent"))?
            .to_path_buf();
        let destination_parent = if parent == APPLICATIONS_URI {
            applications::stored_path(parent)
        } else {
            applications::candidate_path(parent)
        }
        .map_err(provider_error)?;
        if destination_parent != expected_parent {
            return Err(ProviderError::invalid_params(
                "Unsupported: moving application items requires TransferEngine",
            ));
        }
        let renamed = applications::rename_stored(uri, name).map_err(provider_error)?;
        applications::stored_entry(&renamed)
            .map(shared_entry)
            .map_err(provider_error)
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        let uri = request.node_id().unwrap_or(path(request.ctx())?);
        applications::remove_user(uri).map_err(provider_error)
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        search_listing(path(request.ctx())?, request, storage::applications_list)
    }
}
