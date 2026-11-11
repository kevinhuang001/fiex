use super::super::native;
use super::*;
fn static_presentation(
    resolver: &Resolver,
    resource: &crate::plugins::resource::resource_context::ResourceSnapshot,
) -> Result<InfoProviderPresentation, String> {
    match resolver.plugin.as_deref() {
        None => native::presentation(&resolver.id, resource),
        Some(plugin) => {
            let info = plugin
                .extensions
                .get("info")
                .and_then(serde_json::Value::as_object)
                .ok_or("Info provider has no statically registered presentation")?;
            let presentation = serde_json::json!({
                "title": info.get("title"),
                "subsections": info.get("subsections"),
            });
            serde_json::from_value(presentation)
                .map_err(|error| format!("invalid registered Info presentation: {error}"))
        }
    }
}

fn section_view(resolver: &Resolver, presentation: &InfoProviderPresentation) -> InfoSectionView {
    let fields = presentation
        .subsections
        .iter()
        .flat_map(|subsection| {
            subsection.fields.iter().map(move |field| InfoFieldView {
                id: field.id.clone(),
                subsection_id: subsection.id.clone(),
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
        })
        .collect();
    InfoSectionView {
        id: resolver.id.clone(),
        title: presentation.title.clone(),
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
        id: resolver.id.clone(),
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

fn validate_presentation(presentation: &InfoProviderPresentation) -> Result<(), String> {
    if presentation.title.trim().is_empty() || presentation.title.len() > 256 {
        return Err("Info provider registered an invalid title".into());
    }
    if presentation.subsections.len() > 64 {
        return Err("Info provider registered too many subsections".into());
    }
    let mut subsection_ids = HashSet::new();
    let mut total_fields = 0;
    for subsection in &presentation.subsections {
        if !valid_id(&subsection.id)
            || subsection.id.len() > 128
            || subsection.fields.is_empty()
            || !subsection_ids.insert(&subsection.id)
        {
            return Err("Info provider registered an invalid or duplicate subsection".into());
        }
        validate_subsection_match(subsection.r#match.as_ref())?;
        let mut field_ids = HashSet::new();
        for field in &subsection.fields {
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

fn validate_subsection_match(value: Option<&InfoSubsectionMatch>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.extensions.len() > 64
        || value.mime_types.len() > 64
        || value.kinds.len() > 8
        || has_duplicate_match_values(&value.extensions, |item| {
            item.trim_start_matches('.').to_ascii_lowercase()
        })
        || has_duplicate_match_values(&value.mime_types, str::to_ascii_lowercase)
        || has_duplicate_match_values(&value.kinds, str::to_ascii_lowercase)
        || value.extensions.iter().any(|item| {
            let normalized = item.trim_start_matches('.');
            normalized.is_empty()
                || !normalized.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+')
                })
        })
        || value
            .mime_types
            .iter()
            .any(|item| !valid_mime_pattern(item))
        || value
            .kinds
            .iter()
            .any(|kind| !matches!(kind.as_str(), "file" | "directory" | "symlink"))
    {
        return Err("Info subsection has an invalid match rule".into());
    }
    Ok(())
}

fn has_duplicate_match_values(values: &[String], normalize: impl Fn(&str) -> String) -> bool {
    let mut seen = HashSet::new();
    values.iter().any(|value| !seen.insert(normalize(value)))
}

fn valid_mime_pattern(value: &str) -> bool {
    let Some((kind, subtype)) = value.split_once('/') else {
        return false;
    };
    !kind.is_empty()
        && !subtype.is_empty()
        && value.matches('/').count() == 1
        && kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && (subtype == "*"
            || subtype.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+')
            }))
}

fn filter_info_subsections(
    mut presentation: InfoProviderPresentation,
    resource: &crate::plugins::resource::resource_context::ResourceSnapshot,
) -> InfoProviderPresentation {
    presentation.subsections.retain(|subsection| {
        let Some(rule) = subsection.r#match.as_ref() else {
            return true;
        };
        resource
            .metadata()
            .is_some_and(|metadata| rule.matches(metadata))
    });
    presentation
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}
