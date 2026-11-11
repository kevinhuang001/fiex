use super::presentation::*;
use super::*;

#[test]
fn subsection_match_dimensions_are_anded_and_values_are_case_insensitive() {
    let rule = InfoSubsectionMatch {
        extensions: vec!["mp3".into()],
        mime_types: vec!["audio/*".into()],
        kinds: vec!["file".into()],
    };
    let metadata =
        |extension: &str, mime: &str, kind: &str| crate::plugins::contracts::ResourceMetadata {
            extension: Some(extension.into()),
            mime: Some(mime.into()),
            kind: kind.into(),
            ..Default::default()
        };
    assert!(rule.matches(&metadata("MP3", "AUDIO/MPEG", "file")));
    assert!(!rule.matches(&metadata("mp4", "audio/mpeg", "file")));
    assert!(!rule.matches(&metadata("mp3", "video/mp4", "file")));
    assert!(!rule.matches(&metadata("mp3", "audio/mpeg", "directory")));
}

#[test]
fn session_filters_unmatched_subsections_from_trusted_resource_metadata() {
    let metadata = crate::plugins::contracts::ResourceMetadata {
        name: "song.mp3".into(),
        kind: "file".into(),
        extension: Some("MP3".into()),
        mime: Some("audio/mpeg".into()),
        ..Default::default()
    };
    let resource = crate::plugins::resource::resource_context::ResourceSnapshot::from_metadata(
        "local://song.mp3",
        std::path::PathBuf::from("/tmp/song.mp3"),
        "local",
        metadata,
    );
    let field = |id: &str| InfoFieldDescriptor {
        id: id.into(),
        label: id.into(),
        compute: InfoCompute::OnDemand,
    };
    let subsection = |id: &str, extension: &str, field_id: &str| InfoSubsectionDescriptor {
        id: id.into(),
        r#match: Some(InfoSubsectionMatch {
            extensions: vec![extension.into()],
            ..Default::default()
        }),
        fields: vec![field(field_id)],
    };
    let filtered = filter_info_subsections(
        InfoProviderPresentation {
            title: "Media".into(),
            subsections: vec![
                subsection("audio", "mp3", "duration"),
                subsection("video", "mp4", "resolution"),
            ],
        },
        &resource,
    );
    assert_eq!(filtered.title, "Media");
    assert_eq!(filtered.subsections.len(), 1);
    assert_eq!(filtered.subsections[0].id, "audio");
}

#[test]
fn provider_presentation_flattens_multiple_subsections_into_one_section() {
    let resolver = Resolver::builtin("plugin:org.example:media", 50);
    let field = |id: &str| InfoFieldDescriptor {
        id: id.into(),
        label: id.into(),
        compute: InfoCompute::OnDemand,
    };
    let section = section_view(
        &resolver,
        &InfoProviderPresentation {
            title: "Media".into(),
            subsections: vec![
                InfoSubsectionDescriptor {
                    id: "audio".into(),
                    r#match: None,
                    fields: vec![field("duration")],
                },
                InfoSubsectionDescriptor {
                    id: "container".into(),
                    r#match: None,
                    fields: vec![field("duration")],
                },
            ],
        },
    );
    assert_eq!(section.id, resolver.id);
    assert_eq!(section.provider_id, resolver.id);
    assert_eq!(section.title, "Media");
    assert_eq!(section.fields.len(), 2);
    assert_eq!(section.fields[0].subsection_id, "audio");
    assert_eq!(section.fields[1].subsection_id, "container");
}

#[test]
fn rejects_subsection_title_and_invalid_mime_patterns() {
    assert!(
        serde_json::from_value::<InfoSubsectionDescriptor>(serde_json::json!({
            "id": "audio",
            "title": "Audio",
            "fields": [{"id":"duration","label":"Duration","compute":"onDemand"}]
        }))
        .is_err()
    );
    assert!(!valid_mime_pattern("audio/*/bad"));
    assert!(!valid_mime_pattern("*/mpeg"));
    assert!(valid_mime_pattern("audio/*"));
}
