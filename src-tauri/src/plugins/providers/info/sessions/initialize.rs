use super::super::native;
use super::*;
use std::collections::HashSet;

pub(crate) struct InfoComputeWork {
    pub session_id: String,
    pub provider_id: String,
    pub fields: Vec<InfoFieldKey>,
    pub task_id: String,
}

pub(crate) struct OpenedInfoSession {
    pub result: InfoOpenResult,
    pub on_open: Vec<InfoComputeWork>,
}

pub fn initialize_session(path: String) -> Result<OpenedInfoSession, String> {
    let resource =
        crate::plugins::resource::resource_context::resolve_from_directory_snapshot(&path)?;
    let _ = crate::core::fs::trusted_snapshot(&resource, "core:info")?;
    let targets =
        crate::plugins::events::matching::match_resources(std::slice::from_ref(&resource));
    let builtins = native::candidates(&resource);
    let resolvers =
        crate::plugins::provider_registry::candidates("info.compute", &targets, "", builtins);
    let mut providers = HashMap::new();
    let mut sections = Vec::new();
    let mut on_open = Vec::new();

    for resolver in resolvers {
        let descriptors = match static_sections(&resolver, &resource) {
            Ok(sections) => sections,
            Err(error) => {
                sections.push(error_section(&resolver, &error));
                continue;
            }
        };
        if let Err(error) = validate_descriptors(&descriptors) {
            sections.push(error_section(&resolver, &error));
            continue;
        }
        if descriptors.is_empty() {
            continue;
        }

        for descriptor in &descriptors {
            sections.push(section_view(&resolver, descriptor));
        }
        let fields = descriptors
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
        if !fields.is_empty() {
            on_open.push((resolver.id.clone(), fields));
        }
        providers.insert(
            resolver.id.clone(),
            ProviderState {
                resolver,
                sections: descriptors,
            },
        );
    }

    let session_id = new_id("info");
    let resource_version = version(&resource);
    let session = Session {
        resource,
        version: resource_version,
        opened: Instant::now(),
        sections: sections.clone(),
        providers,
        tasks: HashMap::new(),
        revision: 0,
    };
    insert_session(session_id.clone(), session)?;

    Ok(OpenedInfoSession {
        result: InfoOpenResult {
            session_id: session_id.clone(),
            sections,
        },
        on_open: on_open
            .into_iter()
            .map(|(provider_id, fields)| InfoComputeWork {
                session_id: session_id.clone(),
                provider_id,
                fields,
                task_id: new_id("info-open"),
            })
            .collect(),
    })
}

fn static_sections(
    resolver: &Resolver,
    resource: &crate::plugins::resource::resource_context::ResourceSnapshot,
) -> Result<Vec<InfoSectionDescriptor>, String> {
    match resolver.plugin.as_deref() {
        None => native::sections(&resolver.id, resource),
        Some(plugin) => {
            let sections = plugin
                .extensions
                .get("info")
                .and_then(|info| info.get("sections"))
                .ok_or("Info provider has no statically registered sections")?;
            serde_json::from_value(sections.clone())
                .map_err(|error| format!("invalid registered Info sections: {error}"))
        }
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
        return Err("Info provider registered too many sections".into());
    }
    let mut section_ids = HashSet::new();
    let mut total_fields = 0;
    for section in sections {
        if !valid_id(&section.id)
            || section.id.len() > 128
            || section.title.trim().is_empty()
            || section.title.len() > 1024
            || section.fields.is_empty()
            || !section_ids.insert(&section.id)
        {
            return Err("Info provider registered an invalid or duplicate section".into());
        }
        let mut field_ids = HashSet::new();
        for field in &section.fields {
            total_fields += 1;
            if total_fields > FIELD_LIMIT
                || !valid_id(&field.id)
                || field.id.len() > 128
                || field.label.trim().is_empty()
                || field.label.len() > 1024
                || !field_ids.insert(&field.id)
            {
                return Err("Info provider registered an invalid or duplicate field".into());
            }
        }
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}
