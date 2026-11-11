use super::super::native;
use super::*;

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
    let mut total_fields = 0_usize;

    for resolver in resolvers {
        if sections.len() >= PROVIDER_LIMIT_PER_SESSION {
            break;
        }
        let presentation = match presentation::static_presentation(&resolver, &resource) {
            Ok(presentation) => presentation,
            Err(error) => {
                sections.push(presentation::error_section(&resolver, &error));
                continue;
            }
        };
        if let Err(error) = presentation::validate_presentation(&presentation) {
            sections.push(presentation::error_section(&resolver, &error));
            continue;
        }
        let presentation = presentation::filter_info_subsections(presentation, &resource);
        if presentation.subsections.is_empty() {
            continue;
        }
        let provider_fields = presentation
            .subsections
            .iter()
            .map(|subsection| subsection.fields.len())
            .sum::<usize>();
        if provider_fields == 0 {
            continue;
        }
        if total_fields.saturating_add(provider_fields) > FIELD_LIMIT_PER_SESSION {
            sections.push(presentation::error_section(
                &resolver,
                "Info session has reached its field limit",
            ));
            continue;
        }
        total_fields += provider_fields;

        let fields = presentation
            .subsections
            .iter()
            .flat_map(|subsection| {
                subsection
                    .fields
                    .iter()
                    .filter(|field| field.compute == InfoCompute::OnOpen)
                    .map(move |field| InfoFieldKey {
                        subsection_id: subsection.id.clone(),
                        field_id: field.id.clone(),
                    })
            })
            .collect::<Vec<_>>();
        if !fields.is_empty() {
            on_open.push((resolver.id.clone(), fields));
        }
        sections.push(presentation::section_view(&resolver, &presentation));
        providers.insert(
            resolver.id.clone(),
            ProviderState {
                resolver,
                presentation,
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
