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
