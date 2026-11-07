use super::*;

#[test]
fn metadata_descriptors_keep_file_and_directory_attributes_separate() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, b"abc").unwrap();
    let directory = crate::plugins::resource::resource_context::new_resource_snapshot(
        dir.path().to_str().unwrap(),
        dir.path(),
    );
    let file_resource = crate::plugins::resource::resource_context::new_resource_snapshot(
        file.to_str().unwrap(),
        &file,
    );
    let provider = native::provider("core.metadata").unwrap();

    let describe = |resource: &crate::plugins::resource::resource_context::ResourceSnapshot| {
        let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
            "info.describe",
            serde_json::json!({}),
            std::slice::from_ref(resource),
            None,
        );
        provider.describe(&invocation).unwrap()
    };
    let file_sections = describe(&file_resource);
    let directory_sections = describe(&directory);
    let file_basic = file_sections
        .iter()
        .find(|section| section.id == "basic")
        .unwrap();
    let directory_basic = directory_sections
        .iter()
        .find(|section| section.id == "basic")
        .unwrap();
    assert!(file_basic.fields.iter().any(|field| field.id == "size"));
    assert!(!directory_basic
        .fields
        .iter()
        .any(|field| field.id == "size"));
    assert!(directory_sections
        .iter()
        .any(|section| section.id == "directoryStats"));
    assert!(!file_sections
        .iter()
        .any(|section| section.id == "directoryStats"));
}

#[test]
fn metadata_field_compute_returns_only_the_requested_values() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, b"abc").unwrap();
    let resource = crate::plugins::resource::resource_context::new_resource_snapshot(
        file.to_str().unwrap(),
        &file,
    );
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        "info.compute",
        serde_json::json!({"fields":[{"sectionId":"basic","fieldId":"size"}]}),
        std::slice::from_ref(&resource),
        None,
    );
    let result = native::provider("core.metadata")
        .unwrap()
        .compute(
            &invocation,
            &[InfoFieldKey {
                section_id: "basic".into(),
                field_id: "size".into(),
            }],
        )
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value.as_deref(), Some("3 B (3 bytes)"));
}

#[test]
fn timestamp_format_is_stable() {
    assert_eq!(timestamp_text(0), "1970-01-01 00:00");
}
