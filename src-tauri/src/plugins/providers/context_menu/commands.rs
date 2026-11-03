use super::*;

pub fn children(session_id: &str, parent_id: &str) -> Result<Value, String> {
    let (provider, remote_session_id, directory, resources, depth) = {
        let registry = sessions().lock().map_err(|_| "menu registry unavailable")?;
        let session = registry.get(session_id).ok_or("menu session expired")?;
        if session.created.elapsed().as_secs() >= 90 || !allowed(session) {
            return Err("menu session expired".into());
        }
        if session.provider.plugin.is_none() {
            return Err("builtin submenu is not lazy".into());
        }
        (
            session.provider.clone(),
            session
                .remote_session_id
                .clone()
                .ok_or("plugin session missing")?,
            session.directory.clone(),
            session.resources.clone(),
            *session.submenus.get(parent_id).ok_or("unknown submenu")?,
        )
    };
    let params = json!({"menuSessionId":remote_session_id,"parentId":parent_id,
        "interactionId":crate::plugins::mounts::new_id()?});
    let request = ProviderRequest {
        method: "contextMenu.resolve".into(),
        params,
    };
    let (implementation, remote_session) = provider_for(&provider, &directory, &resources);
    let candidate = ProviderCandidate {
        resolver: provider.clone(),
        implementation,
    };
    let invocation = NativeProviderInvocation::new(
        request.method.clone(),
        request.params.clone(),
        resources.as_slice(),
        None,
    );
    let result = EventRouter::resolve_shared(
        "contextMenu.resolve",
        vec![candidate],
        &invocation,
        |resolver, implementation, invocation| {
            let items = implementation
                .resolve(invocation)
                .map_err(|error| error.to_string())?;
            let mut result = json!({"items":items});
            if resolver.plugin.is_some() {
                let session = remote_session
                    .as_ref()
                    .ok_or("plugin menu session unavailable")?
                    .lock()
                    .map_err(|_| "plugin menu session unavailable")?
                    .clone()
                    .ok_or("plugin menu session ID is missing")?;
                result["menuSessionId"] = json!(session);
            }
            Ok(result)
        },
        |_, result| {
            crate::plugins::events::validate_result("contextMenu.resolve", &result)?;
            Ok(result)
        },
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "menu provider unavailable".to_string())?
                .1
        },
    )?;
    let validated = validated_submenu(&result, parent_id, depth)?;
    let mut registry = sessions().lock().map_err(|_| "menu registry unavailable")?;
    let session = registry.get_mut(session_id).ok_or("menu session expired")?;
    if session.created.elapsed().as_secs() >= 90 || !allowed(session) {
        return Err("menu session expired".into());
    }
    if validated
        .actions
        .iter()
        .any(|id| session.actions.contains(id) || session.submenus.contains_key(id))
        || validated
            .submenus
            .keys()
            .any(|id| session.actions.contains(id) || session.submenus.contains_key(id))
    {
        return Err("duplicate menu item id".into());
    }
    if session.actions.len()
        + session.submenus.len()
        + validated.actions.len()
        + validated.submenus.len()
        > 512
    {
        return Err("menu session item limit exceeded".into());
    }
    session.actions.extend(validated.actions);
    session.submenus.extend(validated.submenus);
    Ok(json!({"items":validated.items}))
}
pub fn execute(session_id: &str, item_id: &str) -> Result<Value, String> {
    let session = sessions()
        .lock()
        .map_err(|_| "menu registry unavailable")?
        .remove(session_id)
        .ok_or("menu session invalid or already used")?;
    if session.created.elapsed().as_secs() >= 90
        || !session.actions.contains(item_id)
        || !allowed(&session)
    {
        return Err("menu session expired or item was not offered".into());
    }
    let ctx = builtin::Context::load_for_menu(&session.directory, &session.paths, true)?;
    let request = ProviderRequest {
        method: "contextMenu.execute".into(),
        params: json!({
            "menuSessionId": session.remote_session_id.as_deref().unwrap_or(session_id),
            "itemId": item_id,
            "interactionId": crate::plugins::mounts::new_id()?,
        }),
    };
    let (implementation, _) =
        provider_for(&session.provider, &session.directory, &session.resources);
    let candidate = ProviderCandidate {
        resolver: session.provider.clone(),
        implementation,
    };
    let invocation = file_ops::invocation(&ctx, &request.method, request.params.clone());
    let result = EventRouter::resolve_shared(
        "contextMenu.execute",
        vec![candidate],
        &invocation,
        |resolver, provider, invocation| execute_provider(resolver, provider, invocation),
        |_, result| {
            let wire = serde_json::to_value(&result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("contextMenu.execute", &wire)?;
            Ok(result)
        },
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "menu owner unavailable".to_string())?
                .1
        },
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}
