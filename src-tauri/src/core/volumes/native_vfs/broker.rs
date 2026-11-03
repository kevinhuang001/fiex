use super::*;

fn basename(path: &str) -> Result<String, String> {
    let path = if applications::is_user(path) {
        applications::candidate_path(path)?
    } else {
        PathBuf::from(path)
    };
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "InvalidVfsUri: node has no name".into())
}

pub fn stat_for(broker: &crate::core::fs::ResourceBroker) -> Result<Entry, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.stat",
        json!({"nodeId":broker.path.as_str()}),
    );
    let request = FsRequest::new(&invocation, FsOperation::Stat).map_err(provider_error_text)?;
    provider
        .stat(&request)
        .map(core_entry)
        .map_err(provider_error_text)
}

pub fn list_for(broker: &crate::core::fs::ResourceBroker) -> Result<Vec<Entry>, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.list",
        json!({"nodeId":broker.path.as_str(),"hidden":true}),
    );
    let request = FsRequest::new(&invocation, FsOperation::List).map_err(provider_error_text)?;
    provider
        .list(&request)
        .map(|listing| listing.entries.into_iter().map(core_entry).collect())
        .map_err(provider_error_text)
}

pub fn require_for(
    broker: &crate::core::fs::ResourceBroker,
    operation: FsOperation,
    create: bool,
) -> Result<(), String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.discover",
        json!({"operation":operation.as_str(),"create":create}),
    );
    provider
        .authorize(&invocation, operation, create)
        .map_err(provider_error_text)?;
    Ok(())
}

pub fn preflight_for(
    broker: &crate::core::fs::ResourceBroker,
    destination: Option<&crate::core::fs::ResourceBroker>,
    request: &fiex_provider_contracts::PreflightRequest,
) -> Result<(), String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let params = serde_json::to_value(request).map_err(|error| error.to_string())?;
    let invocation = shared::broker_invocation(broker, destination, "vfs.preflight", params);
    let handlers = provider.preflight_handlers().map_err(provider_error_text)?;
    if !handlers.supports(request.op()) {
        Ok(())
    } else {
        let decision = handlers
            .dispatch(&invocation, request)
            .map_err(provider_error_text)?;
        if decision.allowed {
            Ok(())
        } else {
            Err(format!(
                "{}: {}",
                decision.code.as_deref().unwrap_or("PermissionDenied"),
                decision
                    .reason
                    .as_deref()
                    .unwrap_or("denied by provider policy")
            ))
        }
    }
}

pub fn read_for(broker: &crate::core::fs::ResourceBroker) -> Result<FsRead, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.openRead",
        json!({"nodeId":broker.path.as_str()}),
    );
    let request = FsRequest::new(&invocation, FsOperation::Read).map_err(provider_error_text)?;
    provider.read(&request).map_err(provider_error_text)
}

pub fn write_sized_for(
    broker: &crate::core::fs::ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<FsWrite, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let params = if create {
        let parent = crate::core::fs::parent_path(&broker.path)?;
        let name = basename(&broker.path)?;
        json!({"parentId":parent,"name":name,"size":size})
    } else {
        json!({"nodeId":broker.path.as_str(),"size":size})
    };
    let invocation = shared::broker_invocation(broker, None, "vfs.beginWrite", params);
    let request = FsRequest::new(&invocation, FsOperation::Write).map_err(provider_error_text)?;
    provider.write(&request).map_err(provider_error_text)
}

pub fn mkdir_for(broker: &crate::core::fs::ResourceBroker) -> Result<FsEntry, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let parent = crate::core::fs::parent_path(&broker.path)?;
    let name = basename(&broker.path)?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.mkdir",
        json!({"parentId":parent,"name":name}),
    );
    let request = FsRequest::new(&invocation, FsOperation::Mkdir).map_err(provider_error_text)?;
    provider.mkdir(&request).map_err(provider_error_text)
}

pub fn rename_for(
    source: &crate::core::fs::ResourceBroker,
    destination: &crate::core::fs::ResourceBroker,
) -> Result<FsEntry, String> {
    let source_id =
        shared_provider_id(&source.path).ok_or("Unsupported: unregistered native VFS provider")?;
    if Some(source_id) != shared_provider_id(&destination.path) {
        return Err("Unsupported: cross-provider rename requires TransferEngine".into());
    }
    let provider = registry()
        .get(source_id)
        .ok_or("Unsupported: unregistered native VFS provider")?;
    let parent = crate::core::fs::parent_path(&destination.path)?;
    let name = basename(&destination.path)?;
    let invocation = shared::broker_invocation(
        source,
        Some(destination),
        "vfs.rename",
        json!({"nodeId":source.path.as_str(),"parentId":parent,"name":name}),
    );
    let request = FsRequest::new(&invocation, FsOperation::Rename).map_err(provider_error_text)?;
    provider.rename(&request).map_err(provider_error_text)
}

pub fn remove_for(broker: &crate::core::fs::ResourceBroker) -> Result<(), String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.remove",
        json!({"nodeId":broker.path.as_str()}),
    );
    let request = FsRequest::new(&invocation, FsOperation::Remove).map_err(provider_error_text)?;
    provider.remove(&request).map_err(provider_error_text)
}

pub fn search_for(
    broker: &crate::core::fs::ResourceBroker,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Entry>, bool), String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.search",
        json!({"nodeId":broker.path.as_str(),"query":query,"hidden":hidden,"ignores":ignores,"limit":limit}),
    );
    let request = FsRequest::new(&invocation, FsOperation::Search).map_err(provider_error_text)?;
    let listing = provider
        .search_cancellable(&request, cancelled)
        .map_err(provider_error_text)?;
    Ok((
        listing.entries.into_iter().map(core_entry).collect(),
        listing.truncated,
    ))
}
