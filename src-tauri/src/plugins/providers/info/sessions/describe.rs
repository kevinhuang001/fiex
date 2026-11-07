use super::*;
use std::collections::HashSet;

pub fn describe_session(
    session_id: &str,
    mut on_partial: impl FnMut(Vec<InfoSectionView>),
) -> Result<Vec<InfoSectionView>, String> {
    let resource = {
        let store = sessions()
            .lock()
            .map_err(|_| "Info session store unavailable")?;
        store
            .get(session_id)
            .filter(|session| session.opened.elapsed() < SESSION_TTL)
            .map(|session| session.resource.clone())
            .ok_or("Info session expired")?
    };
    let targets =
        crate::plugins::events::matching::match_resources(std::slice::from_ref(&resource));
    let builtin = native::candidates(&resource);
    let resolvers =
        crate::plugins::provider_registry::candidates("info.describe", &targets, "", builtin);
    let mut providers = HashMap::new();
    let mut sections = Vec::new();

    for resolver in resolvers {
        let descriptions = match describe_provider(&resolver, &resource) {
            Ok(value) => value,
            Err(error) => {
                let section = error_section(&resolver, &error);
                sections.push(section.clone());
                on_partial(vec![section]);
                continue;
            }
        };
        validate_descriptors(&descriptions)?;
        let mut views = Vec::new();
        for descriptor in &descriptions {
            views.push(section_view(&resolver, descriptor));
        }
        if !views.is_empty() {
            on_partial(views.clone());
        }
        sections.extend(views);
        providers.insert(
            resolver.id.clone(),
            ProviderState {
                resolver,
                sections: descriptions,
            },
        );
    }

    {
        let mut store = sessions()
            .lock()
            .map_err(|_| "Info session store unavailable")?;
        let session = store.get_mut(session_id).ok_or("Info session expired")?;
        session.sections = sections;
        session.providers = providers;
    }

    let on_open: Vec<(String, Vec<InfoFieldKey>)> = {
        let store = sessions()
            .lock()
            .map_err(|_| "Info session store unavailable")?;
        let session = store.get(session_id).ok_or("Info session expired")?;
        session
            .providers
            .iter()
            .filter_map(|(provider_id, provider)| {
                let fields = provider
                    .sections
                    .iter()
                    .flat_map(|section| {
                        section
                            .fields
                            .iter()
                            .filter(|field| field.compute == InfoCompute::OnOpen)
                            .map(move |field| InfoFieldKey {
                                section_id: section.id.clone(),
                                field_id: field.id.clone(),
                            })
                    })
                    .collect::<Vec<_>>();
                (!fields.is_empty()).then(|| (provider_id.clone(), fields))
            })
            .collect()
    };
    for (provider_id, fields) in on_open {
        if fields.len() > FIELD_LIMIT {
            continue;
        }
        let result = compute_fields(
            session_id,
            &provider_id,
            &fields,
            &new_id("info-open"),
            false,
        );
        if let Err(error) = result {
            mark_fields_error(session_id, &provider_id, &fields, &error)?;
        }
        on_partial(read_sections(session_id)?);
    }
    read_sections(session_id)
}

fn describe_provider(
    resolver: &Resolver,
    resource: &ResourceSnapshot,
) -> Result<Vec<InfoSectionDescriptor>, String> {
    let params = json!({});
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        "info.describe",
        params.clone(),
        std::slice::from_ref(resource),
        None,
    );
    match resolver.plugin.as_deref() {
        None => native::provider(&resolver.id)
            .ok_or("native Info provider disappeared")?
            .describe(&invocation)
            .map_err(|error| error.to_string()),
        Some(plugin) => ScopedInfoProxy::new(
            plugin.clone(),
            Arc::new(resource.clone()),
            resolver.id.clone(),
        )?
        .describe(&invocation)
        .map_err(|error| error.to_string()),
    }
}

fn section_view(resolver: &Resolver, descriptor: &InfoSectionDescriptor) -> InfoSectionView {
    let fields = descriptor
        .fields
        .iter()
        .map(|field| InfoFieldView {
            id: field.id.clone(),
            section_id: descriptor.id.clone(),
            label: field.label.clone(),
            compute: field.compute,
            status: if field.compute == InfoCompute::OnDemand {
                "idle"
            } else {
                "pending"
            }
            .into(),
            value: None,
            error: None,
        })
        .collect();
    InfoSectionView {
        id: format!("{}::{}", resolver.id, descriptor.id),
        title: descriptor.title.clone(),
        provider_id: resolver.id.clone(),
        provider: resolver.name.clone(),
        source: if resolver.plugin.is_some() {
            "plugin"
        } else {
            "core"
        }
        .into(),
        plugin_id: resolver.plugin.as_deref().map(|plugin| plugin.id.clone()),
        fields,
        error: None,
    }
}

fn error_section(resolver: &Resolver, error: &str) -> InfoSectionView {
    InfoSectionView {
        id: format!("{}::error", resolver.id),
        title: resolver.name.clone(),
        provider_id: resolver.id.clone(),
        provider: resolver.name.clone(),
        source: if resolver.plugin.is_some() {
            "plugin"
        } else {
            "core"
        }
        .into(),
        plugin_id: resolver.plugin.as_deref().map(|plugin| plugin.id.clone()),
        fields: Vec::new(),
        error: Some(crate::infrastructure::redact::clean(error, 1500)),
    }
}

fn validate_descriptors(sections: &[InfoSectionDescriptor]) -> Result<(), String> {
    if sections.len() > 64 {
        return Err("Info provider returned too many sections".into());
    }
    let mut section_ids = HashSet::new();
    for section in sections {
        if !valid_id(&section.id)
            || section.id.len() > 128
            || section.title.trim().is_empty()
            || section.title.len() > 1024
            || section.fields.len() > 256
            || !section_ids.insert(&section.id)
        {
            return Err("Info provider returned an invalid or duplicate section".into());
        }
        let mut field_ids = HashSet::new();
        for field in &section.fields {
            if !valid_id(&field.id)
                || field.id.len() > 128
                || field.label.trim().is_empty()
                || field.label.len() > 1024
                || !field_ids.insert(&field.id)
            {
                return Err("Info provider returned an invalid or duplicate field".into());
            }
        }
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}
