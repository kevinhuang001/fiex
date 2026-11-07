use super::*;
use std::collections::HashSet;

pub fn compute_session(
    session_id: String,
    provider_id: String,
    fields: Vec<InfoFieldKey>,
    task_id: String,
    user_initiated: bool,
) -> Result<Vec<InfoSectionView>, String> {
    if task_id.is_empty() || task_id.len() > 128 || fields.is_empty() || fields.len() > FIELD_LIMIT
    {
        return Err("invalid Info compute request".into());
    }
    compute_fields(&session_id, &provider_id, &fields, &task_id, user_initiated)?;
    read_sections(&session_id)
}

pub fn compute_fields(
    session_id: &str,
    provider_id: &str,
    fields: &[InfoFieldKey],
    task_id: &str,
    user_initiated: bool,
) -> Result<(), String> {
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
            if declaration.compute == InfoCompute::OnDemand && !user_initiated {
                return Err("on-demand Info fields require a user action".into());
            }
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        if session
            .tasks
            .insert(task_id.into(), cancelled.clone())
            .is_some()
        {
            return Err("Info task id is already active".into());
        }
        (
            session.resource.clone(),
            session.version.clone(),
            provider,
            cancelled,
        )
    };
    let result = compute_provider(
        &provider_state.resolver,
        &resource,
        fields,
        cancelled.clone(),
        user_initiated,
    );
    let mut store = sessions()
        .lock()
        .map_err(|_| "Info session store unavailable")?;
    let session = store.get_mut(session_id).ok_or("Info session expired")?;
    session.tasks.remove(task_id);
    if cancelled.load(Ordering::Acquire) {
        return Err("Info calculation cancelled".into());
    }
    if session.version != version_before || version(&session.resource) != version_before {
        return Err("Info resource changed during calculation".into());
    }
    let latest = crate::plugins::resource::resource_context::resolve_from_directory_snapshot(
        &resource.display,
    )?;
    if version(&latest) != version_before {
        return Err("Info resource changed during calculation".into());
    }
    let results = result?;
    validate_field_results(fields, &results)?;
    cache_core_results(&resource, &provider_state.resolver.id, fields, &results);
    for section in session
        .sections
        .iter_mut()
        .filter(|section| section.provider_id == provider_id)
    {
        for field in &mut section.fields {
            if let Some(request) = fields.iter().find(|request| {
                request.section_id == field.section_id && request.field_id == field.id
            }) {
                if let Some(value) = results.iter().find(|result| {
                    result.section_id == request.section_id && result.field_id == request.field_id
                }) {
                    field.status = if value.unavailable {
                        "unavailable"
                    } else {
                        "ready"
                    }
                    .into();
                    field.value = value.value.clone();
                    field.error = None;
                }
            }
        }
    }
    Ok(())
}

fn compute_provider(
    resolver: &Resolver,
    resource: &ResourceSnapshot,
    fields: &[InfoFieldKey],
    cancelled: Arc<AtomicBool>,
    user_initiated: bool,
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
    let candidates = crate::plugins::provider_registry::candidates(
        "info.compute",
        &crate::plugins::events::matching::match_resources(std::slice::from_ref(resource)),
        "",
        native::candidates(resource),
    );
    let resolver = candidates
        .into_iter()
        .find(|candidate| candidate.id == resolver.id)
        .ok_or("Info provider no longer matches this resource")?;
    let cached = read_core_cache(resource, &resolver.id, fields);
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
    let params = json!({"fields": missing});
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
    if user_initiated {
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
    let mut result = cached;
    result.extend(computed);
    Ok(result)
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
