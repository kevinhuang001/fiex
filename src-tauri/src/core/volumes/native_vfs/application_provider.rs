//! Applications is a pure virtual tree. Physical paths are resolved only for
//! imported assets; folders and installed applications have stable VFS IDs.
use super::shared::*;
use super::*;

mod preflight;
mod projection;
mod stage;
use fiex_provider_contracts::{
    PreflightHandlers, PreflightPhase, ProviderError, ProviderInvocation, VfsProvider,
};
use preflight::application_preflight_handlers;
use projection::{applications_list, applications_stat};
use stage::ApplicationStage;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

fn application_operations(uri: &str) -> Vec<FsOperation> {
    if applications::pending_target(uri).is_some() {
        return vec![FsOperation::Stat, FsOperation::Write, FsOperation::Mkdir];
    }
    if applications::is_system_app(uri) {
        return applications::system_entry(uri)
            .map(|entry| {
                entry
                    .allowed_ops
                    .iter()
                    .filter_map(|operation| FsOperation::parse(operation))
                    .collect()
            })
            .unwrap_or_default();
    }
    applications::entry_for_uri(uri)
        .map(|entry| {
            entry
                .allowed_ops
                .iter()
                .filter_map(|operation| FsOperation::parse(operation))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) struct ApplicationView;

/// Applications can move nodes within their mount by changing only parent
/// metadata. The transfer engine calls this provider operation before falling
/// back to byte streams.
pub(super) fn move_within_mount(
    sources: &[crate::core::fs::ResourceBroker],
    destination: &crate::core::fs::ResourceBroker,
) -> Result<Vec<String>, String> {
    let mut paths = Vec::with_capacity(sources.len());
    for source in sources {
        let stat = source.stat()?;
        let name = stat["name"]
            .as_str()
            .ok_or("source has no name")?
            .to_owned();
        source.review_move(destination, Some(&name), PreflightPhase::Execute)?;
        paths.push(source.path.clone());
    }
    let moved = applications::move_nodes(&paths, &destination.path)?;
    for (source, new_path) in sources.iter().zip(&moved) {
        if source.path != *new_path {
            crate::core::fs::changes::renamed(&source.path, new_path);
        }
    }
    Ok(moved)
}

/// Imported entities are stored in the flat private asset root. No virtual
/// folder or system application is converted into a host filesystem path.
pub(super) fn backing_path(uri: &str) -> Option<PathBuf> {
    applications::is_user(uri)
        .then(|| applications::asset_path(uri).ok())
        .flatten()
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
        let parent = if uri == APPLICATIONS_URI {
            APPLICATIONS_URI.to_owned()
        } else {
            applications::parent_uri(uri).map_err(provider_error)?
        };
        if (parent == APPLICATIONS_URI || applications::is_folder(&parent))
            && application_operations(&parent).contains(&operation)
        {
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
        let uri = path(request.ctx())?;
        applications_stat(uri, &application_operations(uri))
            .map(shared_entry)
            .map_err(provider_error)
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        applications_list(path(request.ctx())?)
            .map(list_result)
            .map_err(provider_error)
    }

    fn read(&self, request: &FsRequest<'_>) -> Result<FsRead, ProviderError> {
        let uri = request.node_id().unwrap_or(path(request.ctx())?);
        if !applications::is_user(uri) {
            return Err(ProviderError::method_not_found("vfs.openRead"));
        }
        let file_path = applications::asset_path(uri).map_err(provider_error)?;
        let file = std::fs::File::open(&file_path).map_err(provider_error)?;
        let size = file.metadata().map_err(provider_error)?.len();
        Ok(FsRead::new(file, Some(size)))
    }

    fn write(&self, request: &FsRequest<'_>) -> Result<FsWrite, ProviderError> {
        let (parent, name) = match (request.node_id(), request.parent_id(), request.name()) {
            (None, Some(parent), Some(name)) => (parent, name),
            _ => {
                return Err(ProviderError::method_not_found("vfs.beginWrite"));
            }
        };
        let target = applications::reserved_child(parent, name).map_err(provider_error)?;
        if target.kind != applications::PendingKind::Application {
            return Err(ProviderError::invalid_params(
                "Applications writes must target an imported application",
            ));
        }
        let uri = target.uri.clone();
        let destination = applications::asset_path(&uri).map_err(provider_error)?;
        let staging_root = dirs::home_dir()
            .ok_or_else(|| ProviderError::internal("cannot locate home directory"))?
            .join(".fiex")
            .join("application-staging");
        std::fs::create_dir_all(&staging_root)
            .map_err(|error| ProviderError::internal(error.to_string()))?;
        let temporary = tempfile::NamedTempFile::new_in(&staging_root)
            .map_err(|error| ProviderError::internal(error.to_string()))?;
        let staged = Arc::new(Mutex::new(Some(temporary)));
        let committed = Arc::new(AtomicBool::new(false));
        let writer = ApplicationStage {
            file: Arc::clone(&staged),
            uri: uri.clone(),
            committed: Arc::clone(&committed),
        };
        let commit_file = Arc::clone(&staged);
        let commit_flag = Arc::clone(&committed);
        Ok(FsWrite::with_commit(writer, move || {
            let result = (|| {
                let temporary = commit_file
                    .lock()
                    .map_err(|_| ProviderError::internal("write stream unavailable"))?
                    .take()
                    .ok_or_else(|| ProviderError::internal("write stream closed"))?;
                temporary
                    .as_file()
                    .sync_all()
                    .map_err(|error| ProviderError::internal(error.to_string()))?;
                applications::validate_import_asset(temporary.path(), &target.name)
                    .map_err(provider_error)?;
                if destination.exists() {
                    return Err(ProviderError::invalid_params(
                        "Conflict: application asset already exists",
                    ));
                }
                temporary
                    .persist_noclobber(&destination)
                    .map_err(|error| ProviderError::internal(error.error.to_string()))?;
                if let Err(error) =
                    crate::core::volumes::application_policy::finish_import(&uri, true)
                {
                    let _ = std::fs::remove_file(&destination);
                    return Err(provider_error(error));
                }
                match applications::commit_reserved_application(&uri) {
                    Ok(entry) => {
                        commit_flag.store(true, Ordering::Release);
                        crate::core::fs::changes::committed(&uri);
                        Ok(shared_entry(entry))
                    }
                    Err(error) => {
                        let _ = std::fs::remove_file(&destination);
                        Err(provider_error(error))
                    }
                }
            })();
            if result.is_err() {
                applications::cancel_reservation(&uri);
            }
            result
        }))
    }

    fn mkdir(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let uri = path(request.ctx())?;
        applications::commit_reserved_folder(uri)
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
        applications::rename_node(uri, parent, name)
            .map(shared_entry)
            .map_err(provider_error)
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        let uri = request.node_id().unwrap_or(path(request.ctx())?);
        applications::remove_node(uri).map_err(provider_error)
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        search_listing(path(request.ctx())?, request, applications_list)
    }
}
