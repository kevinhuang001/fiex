//! Trusted invocation context and common projections for native VFS providers.
use super::*;
use crate::core::files::Entry;
use crate::plugins::{
    native_invocation::NativeProviderInvocation, resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    FsEntry, FsKind, FsListing, FsRequest, ProviderError, ProviderInvocation, ResourceMetadata,
};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
struct VfsTarget {
    path: String,
    owner: String,
    broker: Option<Arc<crate::core::fs::ResourceBroker>>,
    destination: Option<Arc<crate::core::fs::ResourceBroker>>,
}

pub(super) fn path(ctx: &dyn ProviderInvocation) -> Result<&str, ProviderError> {
    NativeProviderInvocation::local_data::<VfsTarget>(ctx).map(|target| target.path.as_str())
}

pub(super) fn owner(ctx: &dyn ProviderInvocation) -> Result<&str, ProviderError> {
    NativeProviderInvocation::local_data::<VfsTarget>(ctx).map(|target| target.owner.as_str())
}

pub(super) fn broker(
    ctx: &dyn ProviderInvocation,
) -> Result<&crate::core::fs::ResourceBroker, ProviderError> {
    let target = NativeProviderInvocation::local_data::<VfsTarget>(ctx)?;
    target
        .broker
        .as_deref()
        .ok_or_else(|| ProviderError::invalid_params("missing scoped storage broker"))
}

pub(super) fn destination_broker(
    ctx: &dyn ProviderInvocation,
) -> Result<&crate::core::fs::ResourceBroker, ProviderError> {
    let target = NativeProviderInvocation::local_data::<VfsTarget>(ctx)?;
    target
        .destination
        .as_deref()
        .ok_or_else(|| ProviderError::invalid_params("missing scoped destination broker"))
}

pub(super) fn target_invocation(
    path: &str,
    owner: &str,
    method: &str,
    params: Value,
) -> NativeProviderInvocation {
    let kind = if path == RECENT_URI
        || path == APPLICATIONS_URI
        || path == recycle::URI
        || (path.ends_with('/') && (applications::is_user(path) || trash_vfs::is_tree(path)))
    {
        "directory"
    } else {
        "file"
    };
    let target = ResourceSnapshot::from_metadata(
        path,
        ResourceMetadata {
            kind: kind.into(),
            ..Default::default()
        },
    );
    NativeProviderInvocation::new(method, params, std::slice::from_ref(&target), None)
        .with_local_data(Arc::new(VfsTarget {
            path: path.into(),
            owner: owner.into(),
            broker: None,
            destination: None,
        }))
}

pub(super) fn broker_invocation(
    source: &crate::core::fs::ResourceBroker,
    destination: Option<&crate::core::fs::ResourceBroker>,
    method: &str,
    params: Value,
) -> NativeProviderInvocation {
    let mut target = ResourceSnapshot::from_metadata(&source.path, ResourceMetadata::default());
    target.reference = source.reference.clone();
    let mut targets = vec![target];
    if let Some(destination) = destination {
        let mut target =
            ResourceSnapshot::from_metadata(&destination.path, ResourceMetadata::default());
        target.reference = destination.reference.clone();
        targets.push(target);
    }
    NativeProviderInvocation::new(method, params, &targets, None).with_local_data(Arc::new(
        VfsTarget {
            path: source.path.clone(),
            owner: source.owner.clone(),
            broker: Some(Arc::new(source.clone_binding())),
            destination: destination.map(|broker| Arc::new(broker.clone_binding())),
        },
    ))
}

pub(super) fn shared_entry(entry: Entry) -> FsEntry {
    let Entry {
        path,
        name,
        is_dir,
        is_symlink,
        size,
        modified,
        extension,
        mime,
        readonly,
        link_target,
        allowed_ops,
        ..
    } = entry;
    let kind = if is_symlink {
        FsKind::Symlink
    } else if is_dir {
        FsKind::Directory
    } else {
        FsKind::File
    };
    FsEntry {
        // Native view IDs are already opaque view references, never host paths.
        node_id: path,
        name,
        kind,
        size: (!is_dir).then_some(size),
        modified: Some(modified as i64),
        extension: (!extension.is_empty()).then_some(extension),
        mime: (!mime.is_empty()).then_some(mime),
        revision: None,
        readonly: Some(readonly),
        link_target,
        allowed_ops: Some(
            allowed_ops
                .iter()
                .filter_map(|name| FsOperation::parse(name))
                .collect(),
        ),
        relative_path: None,
    }
}

pub fn core_entry(entry: FsEntry) -> Entry {
    Entry {
        path: entry.node_id,
        name: entry.name,
        is_dir: entry.kind == FsKind::Directory,
        is_symlink: entry.kind == FsKind::Symlink,
        size: entry.size.unwrap_or_default(),
        modified: entry.modified.unwrap_or_default().max(0) as u64,
        extension: entry.extension.unwrap_or_default(),
        mime: entry.mime.unwrap_or_default(),
        readonly: entry.readonly.unwrap_or(false),
        link_target: entry.link_target,
        allowed_ops: entry
            .allowed_ops
            .unwrap_or_default()
            .into_iter()
            .map(|operation| operation.as_str().to_owned())
            .collect(),
        ..Default::default()
    }
}

pub(super) fn list_result(entries: Vec<Entry>) -> FsListing {
    FsListing {
        entries: entries.into_iter().map(shared_entry).collect(),
        ..Default::default()
    }
}

pub(super) fn provider_error(error: impl ToString) -> ProviderError {
    let message = error.to_string();
    if message.starts_with("Unsupported:") {
        ProviderError::method_not_found("vfs.operation")
    } else {
        ProviderError::invalid_params(message)
    }
}

pub(super) fn search_listing(
    root: &str,
    request: &FsRequest<'_>,
    mut list: impl FnMut(&str) -> Result<Vec<Entry>, String>,
) -> Result<FsListing, ProviderError> {
    let query = request
        .query()
        .filter(|query| !query.is_empty())
        .ok_or_else(|| ProviderError::invalid_params("search query is required"))?
        .to_lowercase();
    let ignore_patterns = request.ignores();
    let ignores = crate::core::ignores::IgnoreSet::new(&ignore_patterns);
    let mut pending = vec![(root.to_owned(), String::new())];
    let mut visited = std::collections::HashSet::new();
    let mut results = Vec::new();
    let mut truncated = false;
    while let Some((directory, prefix)) = pending.pop() {
        if !visited.insert(directory.clone()) {
            continue;
        }
        if visited.len() > 1_000 {
            truncated = true;
            break;
        }
        for entry in list(&directory).map_err(provider_error)? {
            let relative_path = if prefix.is_empty() {
                entry.name.clone()
            } else {
                format!("{prefix}/{}", entry.name)
            };
            if (!request.hidden() && entry.name.starts_with('.'))
                || ignores.is_ignored(Path::new(&relative_path))
            {
                continue;
            }
            if entry.is_dir && !entry.is_symlink && entry.allowed_ops.iter().any(|op| op == "list")
            {
                pending.push((entry.path.clone(), relative_path.clone()));
            }
            if entry.name.to_lowercase().contains(&query) {
                if results.len() == request.limit() {
                    truncated = true;
                    break;
                }
                let mut result = shared_entry(entry);
                result.relative_path = Some(relative_path);
                results.push(result);
            }
        }
        if truncated {
            break;
        }
    }
    Ok(FsListing {
        entries: results,
        truncated,
        ..Default::default()
    })
}
