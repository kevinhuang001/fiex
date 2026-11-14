use super::*;

/// A plugin result is never authority to run a privileged native command.
/// Only an already-matched Core builtin may return the private nativeAction
/// marker, and the action must be identical to the one Core revalidated.
pub(super) fn native_action_authorized(
    builtin: bool,
    item_id: &str,
    result: &MenuExecuteResult,
) -> Result<bool, String> {
    match (builtin, result) {
        (true, MenuExecuteResult::NativeAction(action)) if action.item_id == item_id => Ok(true),
        (true, _) => Err("invalid native connection menu response".into()),
        (
            false,
            MenuExecuteResult::NativeAction(_)
            | MenuExecuteResult::CoreAction(_)
            | MenuExecuteResult::ConnectionCoreAction(_)
            | MenuExecuteResult::RemoteAction(_),
        ) => Err("plugin cannot execute native connection actions".into()),
        (false, _) => Ok(false),
    }
}

pub async fn execute(
    connection_id: &str,
    provider_id: &str,
    item_id: &str,
    menu_session_id: Option<&str>,
) -> Result<Value, String> {
    let ctx = Context::load(connection_id)?;
    let targets = [ctx.matcher_resource()];
    let owner = crate::plugins::provider_registry::candidates(
        "connectionContextMenu.execute",
        &targets,
        &ctx.info.mount_path,
        providers(&targets),
    )
    .into_iter()
    .find(|provider| provider.id == provider_id)
    .ok_or("menu provider is disabled or does not match connection")?;
    let builtin_owner = owner.plugin.is_none();
    let session = menu_session_id
        .map(str::to_owned)
        .unwrap_or(crate::plugins::mounts::new_id()?);
    let request = ProviderRequest {
        method: "connectionContextMenu.execute".into(),
        params: json!({"itemId":item_id}),
    };
    let implementation = implementation_for(&owner, &ctx, &session);
    let candidate = ProviderCandidate {
        resolver: owner.clone(),
        implementation,
    };
    let invocation = if owner.plugin.is_none() {
        native::invocation(&owner.id, request.method.clone(), request.params.clone())?
            .with_local_data(Arc::new(ctx.clone()))
    } else {
        NativeProviderInvocation::new(request.method.clone(), request.params.clone(), &[], None)
            .with_local_data(Arc::new(ctx.clone()))
    };
    let result = EventRouter::resolve_shared(
        "connectionContextMenu.execute",
        vec![candidate],
        &invocation,
        |resolver, implementation, invocation| {
            if resolver.plugin.is_none() {
                let bound = native::invocation(
                    &resolver.id,
                    request.method.clone(),
                    request.params.clone(),
                )?
                .with_local_data(Arc::new(ctx.clone()));
                implementation
                    .execute(&bound)
                    .map_err(|error| error.to_string())
            } else {
                implementation
                    .execute(invocation)
                    .map_err(|error| error.to_string())
            }
        },
        |_, result| {
            let wire = serde_json::to_value(&result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("connectionContextMenu.execute", &wire)?;
            Ok(result)
        },
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "connection menu provider unavailable".to_string())?
                .1
        },
    )?;
    if native_action_authorized(builtin_owner, item_id, &result)? {
        return native::execute(provider_id, &ctx, item_id)
            .await
            .and_then(|result| serde_json::to_value(result).map_err(|error| error.to_string()));
    }
    serde_json::to_value(result).map_err(|error| error.to_string())
}
