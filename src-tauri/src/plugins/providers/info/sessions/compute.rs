use super::super::{native, proxy::ScopedInfoProxy};
use super::*;
use fiex_provider_contracts::InfoProvider;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComputeTrigger {
    OnOpen,
    OnDemand,
}

pub fn compute_session(
    session_id: String,
    provider_id: String,
    fields: Vec<InfoFieldKey>,
    task_id: String,
) -> Result<InfoFieldDelta, String> {
    compute_fields(
        &session_id,
        &provider_id,
        &fields,
        &task_id,
        ComputeTrigger::OnDemand,
    )
}

pub fn compute_fields(
    session_id: &str,
    provider_id: &str,
    fields: &[InfoFieldKey],
    task_id: &str,
    trigger: ComputeTrigger,
) -> Result<InfoFieldDelta, String> {
    if task_id.is_empty() || task_id.len() > 128 || fields.is_empty() || fields.len() > FIELD_LIMIT
    {
        return Err("invalid Info compute request".into());
    }
    let (resource, version_before, provider_state, cancelled) = {
        let mut store = sessions()
            .lock()
            .map_err(|_| "Info session store unavailable")?;
        let session = store.get_mut(session_id).ok_or("Info session expired")?;
        if session.opened.elapsed() >= SESSION_TTL {
            return Err("Info session expired".into());
        }
        let provider = session
            .providers
            .get(provider_id)
            .cloned()
            .ok_or("Info provider is not part of this session")?;
        for field in fields {
            let declaration = provider
                .sections
                .iter()
                .find(|section| section.id == field.section_id)
                .and_then(|section| section.fields.iter().find(|item| item.id == field.field_id))
                .ok_or("Info field is not declared by this provider")?;
            let expected = match trigger {
                ComputeTrigger::OnOpen => InfoCompute::OnOpen,
                ComputeTrigger::OnDemand => InfoCompute::OnDemand,
            };
            if declaration.compute != expected {
                return Err("Info field does not match the Core compute trigger".into());
            }
        }
        if session.tasks.contains_key(task_id) {
            return Err("Info task id is already active".into());
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        session.tasks.insert(task_id.into(), cancelled.clone());
        (
            session.resource.clone(),
            session.version.clone(),
            provider,
            cancelled,
        )
    };

    let user_action = trigger == ComputeTrigger::OnDemand;
    let mut result = compute_provider(
        &provider_state.resolver,
        &resource,
        fields,
        cancelled.clone(),
        user_action,
    );
    if result.is_ok() {
        let latest = crate::plugins::resource::resource_context::resolve_from_directory_snapshot(
            &resource.display,
        );
        result = match latest {
            Ok(latest) if version(&latest) == version_before => result,
            Ok(_) => Err("Info resource changed during calculation".into()),
            Err(error) => Err(error),
        };
    }
    if cancelled.load(Ordering::Acquire) {
        result = Err("Info calculation cancelled".into());
    }

    {
        let mut store = sessions()
            .lock()
            .map_err(|_| "Info session store unavailable")?;
        let session = store.get_mut(session_id).ok_or("Info session expired")?;
        session.tasks.remove(task_id);
        if session.version != version_before {
            result = Err("Info resource changed during calculation".into());
        }
    }

    match result {
        Ok(results) => {
            validate_field_results(fields, &results)?;
            let allow_plugin_cache = plugin_cache_allowed(&provider_state.resolver);
            cache_core_results(
                &resource,
                &provider_state.resolver.id,
                fields,
                &results,
                allow_plugin_cache,
            );
            update_fields(session_id, provider_id, fields, &results, None)
        }
        Err(error) => mark_fields_error(session_id, provider_id, fields, &error),
    }
}

fn compute_provider(
    resolver: &Resolver,
    resource: &ResourceSnapshot,
    fields: &[InfoFieldKey],
    cancelled: Arc<AtomicBool>,
    user_action: bool,
) -> Result<Vec<InfoFieldResult>, String> {
    if fields
        .iter()
        .any(|field| field.section_id == "directoryStats")
    {
        if crate::core::fs::provider_registry::resource_access(&resource.display)
            .map_err(String::from)?
            != crate::core::fs::provider_registry::ResourceAccess::HostPath
        {
            return Err("directory statistics are available for local folders only".into());
        }
        crate::plugins::resource::file_capabilities::require(
            &resource.display,
            crate::plugins::provider_registry::FsOperation::List,
        )?;
    }
    if !provider_still_matches(resolver, resource) {
        return Err("Info provider no longer matches this resource".into());
    }
    let allow_plugin_cache = plugin_cache_allowed(resolver);
    let cached = read_core_cache(resource, &resolver.id, fields, allow_plugin_cache);
    if cached.len() == fields.len() {
        return Ok(cached);
    }
    let missing = fields
        .iter()
        .filter(|field| {
            !cached.iter().any(|value| {
                value.section_id == field.section_id && value.field_id == field.field_id
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let _permit = ComputePermit::acquire(&cancelled)?;
    let params = serde_json::json!({"fields": missing});
    let mut invocation = if resolver.plugin.is_none() {
        native::invocation_with_cancel(
            &resolver.id,
            "info.compute",
            params.clone(),
            resource,
            cancelled.clone(),
        )?
    } else {
        crate::plugins::native_invocation::NativeProviderInvocation::new(
            "info.compute",
            params,
            std::slice::from_ref(resource),
            None,
        )
    };
    if user_action {
        invocation = invocation.with_user_action_context(
            crate::core::fs::mapped_access::UserActionContext::from_core_event(),
        );
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("Info calculation cancelled".into());
    }
    let computed = match resolver.plugin.as_deref() {
        None => native::provider(&resolver.id)
            .ok_or("native Info provider disappeared")?
            .compute(&invocation, &missing),
        Some(plugin) => ScopedInfoProxy::new(
            plugin.clone(),
            Arc::new(resource.clone()),
            resolver.id.clone(),
        )?
        .compute(&invocation, &missing),
    }
    .map_err(|error| error.to_string())?;
    if cancelled.load(Ordering::Acquire) {
        return Err("Info calculation cancelled".into());
    }
    let mut result = cached;
    result.extend(computed);
    Ok(result)
}

fn provider_still_matches(resolver: &Resolver, resource: &ResourceSnapshot) -> bool {
    match resolver.plugin.as_deref() {
        None => native::candidates(resource)
            .iter()
            .any(|candidate| candidate.id == resolver.id),
        Some(plugin) => {
            plugin.enabled
                && plugin.approved
                && plugin.backend_available
                && crate::plugins::events::matching::matches_resources(
                    "info.compute",
                    &plugin.extensions,
                    std::slice::from_ref(resource),
                    None,
                )
        }
    }
}

fn plugin_cache_allowed(resolver: &Resolver) -> bool {
    let Some(plugin) = resolver.plugin.as_deref() else {
        return true;
    };
    if !plugin
        .declared_permissions
        .iter()
        .any(|permission| permission == "files.read")
    {
        return true;
    }
    let declared = plugin.declared_permissions.iter().cloned().collect();
    crate::plugins::security::policy::PermissionPolicy::new(&plugin.id, &declared)
        .allowed(crate::plugins::security::policy::Capability::FilesRead)
}

fn validate_field_results(
    requested: &[InfoFieldKey],
    results: &[InfoFieldResult],
) -> Result<(), String> {
    if results.len() != requested.len() {
        return Err("Info provider did not return exactly the requested fields".into());
    }
    let mut returned = HashSet::new();
    for value in results {
        if value.section_id.len() > 128
            || value.field_id.len() > 128
            || value
                .value
                .as_ref()
                .is_some_and(|value| value.len() > 16_384)
            || value.unavailable && value.value.is_some()
            || !value.unavailable && value.value.is_none()
            || !returned.insert((value.section_id.as_str(), value.field_id.as_str()))
        {
            return Err("Info provider returned an invalid field result".into());
        }
        if !requested
            .iter()
            .any(|field| field.section_id == value.section_id && field.field_id == value.field_id)
        {
            return Err("Info provider returned an unrequested field".into());
        }
    }
    Ok(())
}
