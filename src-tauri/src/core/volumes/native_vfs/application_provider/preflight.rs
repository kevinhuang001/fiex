//! Applications-specific preflight validation for virtual-tree operations.
use super::*;
use fiex_provider_contracts::{
    CreateFilePreflight, ExportPreflight, ImportPreflight, MovePreflight, PreflightDecision,
    PreflightHandlers, PreflightPhase, ProviderError, ProviderInvocation, RemovePreflight,
    RenamePreflight,
};
use std::sync::LazyLock;

pub(super) fn application_preflight_handlers() -> PreflightHandlers {
    static HANDLERS: LazyLock<PreflightHandlers> = LazyLock::new(|| {
        let mut handlers = PreflightHandlers::new();
        handlers
            .on_create_file(check_application_create_file)
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

fn check_application_create_file(
    _ctx: &dyn ProviderInvocation,
    _phase: PreflightPhase,
    request: &CreateFilePreflight,
) -> Result<PreflightDecision, ProviderError> {
    let allowed = request.name.as_deref().is_some_and(|name| {
        applications::reserved_child(&request.parent_id, name)
            .is_ok_and(|target| target.kind == applications::PendingKind::Application)
    });
    if allowed {
        Ok(PreflightDecision::allow())
    } else {
        Ok(PreflightDecision::deny(
            "Applications accepts only validated application imports",
        ))
    }
}

fn check_application_import(
    _ctx: &dyn ProviderInvocation,
    _phase: PreflightPhase,
    request: &ImportPreflight,
) -> Result<PreflightDecision, ProviderError> {
    if request.source.system == Some(true) || request.source.executable_shortcut != Some(true) {
        return Ok(PreflightDecision::deny(
            "Applications accepts only verified applications or shortcuts",
        ));
    }
    let Some(name) = request.name.as_deref() else {
        return Ok(PreflightDecision::deny("application name is required"));
    };
    if applications::reserved_child(&request.parent_id, name).is_err() {
        return Ok(PreflightDecision::deny(
            "Applications import target was not reserved by Core",
        ));
    }
    Ok(PreflightDecision::allow())
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
    if applications::is_user(&source.path) {
        Ok(PreflightDecision::allow())
    } else {
        Ok(PreflightDecision::deny(
            "Only imported applications can leave the Applications view",
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
    if applications::is_system_app(&source.path)
        || (!applications::is_user(&source.path) && !applications::is_folder(&source.path))
    {
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
    match applications::validate_remove(&source.path) {
        Ok(()) => Ok(PreflightDecision::allow()),
        Err(reason) => Ok(PreflightDecision::deny(reason)),
    }
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
    if !applications::is_user(&source.path)
        && !applications::is_folder(&source.path)
        && !applications::is_system_app(&source.path)
    {
        return Ok(PreflightDecision::deny(
            "Only Applications entries can be moved inside Applications",
        ));
    }
    if target_parent.path != APPLICATIONS_URI && !applications::is_folder(&target_parent.path) {
        return Ok(PreflightDecision::deny(
            "Applications entries can only be moved within Applications",
        ));
    }
    Ok(PreflightDecision::allow())
}
