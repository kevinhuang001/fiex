use super::session::{validate, UNSUPPORTED};
use super::*;

#[test]
fn explicit_unsupported_view_is_not_a_failure() {
    let error = validate("org.fiex.test", serde_json::json!({"kind":"unsupported"})).unwrap_err();
    assert_eq!(error, UNSUPPORTED);
}

#[test]
fn unknown_preview_kind_is_rejected() {
    let error = validate(
        "org.fiex.test",
        serde_json::json!({"kind":"iframe","view":{"source":"package","entry":"ui/x.html"}}),
    )
    .unwrap_err();
    assert!(error.contains("invalid PreviewView") || error.contains("unsupported"));
}

#[test]
fn builtins_are_ordered_and_fallback_is_last() {
    let values = candidates("md", "text/markdown", false);
    assert_eq!(values[0].id, "builtin:text");
    assert!(!values
        .iter()
        .any(|provider| provider.id == "builtin:fallback"));
    let pdf = candidates("pdf", "application/pdf", false);
    assert!(pdf.iter().any(|provider| provider.id == "builtin:pdf"));
    let unknown = candidates("xyz", "application/octet-stream", false);
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].id, "builtin:fallback");
}

#[test]
fn preview_size_ceiling_applies_to_remote_resource_metadata() {
    let virtual_file = ResourceSnapshot::from_metadata(
        "fiex://mount/remote/example.bin",
        crate::plugins::contracts::ResourceMetadata {
            kind: "file".into(),
            size: Some(64 * 1024 * 1024),
            ..Default::default()
        },
    );
    assert!(virtual_file.stat.is_none());
    assert_eq!(resource_size(&virtual_file), Some(64 * 1024 * 1024));
    assert!(resource_size(&virtual_file).unwrap() > 16 * 1024 * 1024);
}
