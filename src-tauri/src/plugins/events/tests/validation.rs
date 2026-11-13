use super::super::*;

#[test]
fn info_compute_rejects_invalid_field_results() {
    assert!(
        validate_result(
            "info.compute",
            &json!({"fields":[{"sectionId":"basic","fieldId":"size","value":3}]})
        )
        .is_err()
    );
    assert!(
        validate_result(
            "info.compute",
            &json!({"fields":[
                {"subsectionId":"basic","fieldId":"size","value":"1 KiB"},
                {"subsectionId":"basic","fieldId":"size","value":"2 KiB"}
            ]})
        )
        .is_err()
    );
}

#[test]
fn connection_validation_rejects_untyped_or_unknown_fields() {
    assert!(validate_result("connection.validate", &json!({"valid": true})).is_ok());
    assert!(validate_result("connection.validate", &json!({"valid": "true"})).is_err());
    assert!(
        validate_result(
            "connection.validate",
            &json!({"valid": true, "credentials": {}, "unexpected": true})
        )
        .is_err()
    );
}

#[test]
fn fixed_events_are_core_owned_custom_methods_remain_available() {
    for method in [
        "info.compute",
        "preview.open",
        "contextMenu.resolve",
        "search.query",
        "toolbar.invoke",
        "hooks.invoke",
        "fs.openRead",
    ] {
        assert!(is_protected_method(method), "{method}");
    }
    for method in [
        "checksum.calculate",
        "git.stage",
        "archive.compress",
        "search.status",
        "search.rebuild",
    ] {
        assert!(!is_protected_method(method), "{method}");
    }
}
