use super::*;
use serde_json::json;
#[test]
fn removed_top_level_capabilities_are_not_silently_accepted() {
    let mut manifest = json!({"schemaVersion":2,"id":"test.plugin","name":"Test","version":"1"});
    assert!(serde_json::from_value::<PluginManifest>(manifest.clone()).is_ok());
    manifest["capabilities"] = json!(["connection"]);
    let error = serde_json::from_value::<PluginManifest>(manifest)
        .unwrap_err()
        .to_string();
    assert!(error.contains("capabilities"));
}
#[test]
fn provider_declarations_are_explicit_and_unambiguous() {
    for value in [
        json!({}),
        json!({"vfs": {"title": "History"}}),
        json!({"connections": {"title": "Drive", "fields": []}}),
        json!({"toolbar": {"title": "Git"}}),
        json!({"search": {"icon": "magnifying-glass"}}),
        json!({"hooks": {"event": "beforeOpen"}}),
    ] {
        assert!(validate_extensions(&value).is_ok(), "{value}");
    }
    for value in [
        json!({"vfs": []}),
        json!({"vfs": {}}),
        json!({"vfs": {"id": "history", "title": "History"}}),
        json!({"vfs": [{"title": "History"}]}),
        json!({"connections": {"id": "drive", "title": "Drive", "fields": []}}),
        json!({"connections": [{"title": "Drive", "fields": []}]}),
        json!({"toolbar": {"id": "git.main", "title": "Git"}}),
        json!({"toolbar": {"schemes": ["history"], "title": "Git"}}),
        json!({"search": []}),
        json!({"search": {"id": "search"}}),
        json!({"hooks": []}),
        json!({"hooks": {"id": "beforeOpen", "event": "beforeOpen"}}),
    ] {
        assert!(validate_extensions(&value).is_err(), "{value}");
    }
}
#[test]
fn ui_timeout_is_an_explicit_bounded_contract() {
    assert_eq!(ui_timeout_ms(&json!({})), DEFAULT_UI_TIMEOUT_MS);
    assert_eq!(
        ui_timeout_ms(&json!({"ui": {"timeoutMs": 300_000}})),
        300_000
    );
    // The process request budget keeps its historical default and can only
    // be extended by the manifest contract.
    assert_eq!(request_timeout_ms(&json!({})), DEFAULT_REQUEST_TIMEOUT_MS);
    assert_eq!(
        request_timeout_ms(&json!({"ui": {"timeoutMs": 1_000}})),
        DEFAULT_REQUEST_TIMEOUT_MS
    );
    assert_eq!(
        request_timeout_ms(&json!({"ui": {"timeoutMs": 300_000}})),
        300_000
    );
    for invalid in [
        json!({"ui": []}),
        json!({"ui": {"timeoutMs": "long"}}),
        json!({"ui": {"timeoutMs": 0}}),
        json!({"ui": {"timeoutMs": MAX_UI_TIMEOUT_MS + 1}}),
        json!({"ui": {"timeoutMs": 30_000, "extra": true}}),
    ] {
        assert!(validate_extensions(&invalid).is_err(), "{invalid}");
    }
    assert!(validate_extensions(&json!({"ui": {"timeoutMs": 300_000}})).is_ok());
}

#[test]
fn menu_registration_rejects_static_items_and_nested_syntax() {
    assert!(validate_extensions(&serde_json::json!({"contextMenus": true})).is_ok());
    assert!(validate_extensions(&serde_json::json!({"contextMenus": false})).is_ok());
    for invalid in [
        serde_json::json!({"contextMenus": []}),
        serde_json::json!({"contextMenus": [{"when":"context.selection.count > 0"}]}),
        serde_json::json!({"contextMenus": [{"children":[]}]}),
    ] {
        assert!(validate_extensions(&invalid).is_err());
    }
}
#[test]
fn match_predicates_are_scoped_by_extension_point() {
    assert!(validate_extensions(&serde_json::json!({
        "decorations": {"match": {"contains": [".git"]}}
    }))
    .is_ok());
    assert!(validate_extensions(&serde_json::json!({
        "contextMenus": {"match": {"selection": "single"}}
    }))
    .is_ok());
    assert!(validate_extensions(&serde_json::json!({
        "info": {"match": {"mimeTypes": ["text/*"]}}
    }))
    .is_ok());
    for invalid in [
        serde_json::json!({"decorations": {"match": {"selection": "single"}}}),
        serde_json::json!({"contextMenus": {"match": {"contains": [".git"]}}}),
        serde_json::json!({"info": {"match": {"contains": [".git"]}}}),
        serde_json::json!({"info": [{"id": "x"}]}),
        serde_json::json!({"info": {"title": "X"}}),
        serde_json::json!({"info": {"id": "x"}}),
        serde_json::json!({"contextMenus": {"match": {"selection": "many"}}}),
    ] {
        assert!(validate_extensions(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn preview_loading_pages_are_optional_and_package_local() {
    for valid in [
        json!({"preview": {}}),
        json!({"preview": {"panel": {"entry": "ui/preview.html"}}}),
        json!({"preview": {"loading": {"entry": "ui/loading.html"}}}),
        json!({"preview": {
            "panel": {"entry": "ui/preview.html"},
            "loading": {"entry": "ui/loading.html"}
        }}),
    ] {
        assert!(validate_extensions(&valid).is_ok(), "{valid}");
    }
    for invalid in [
        json!({"preview": {"loading": {}}}),
        json!({"preview": {"loading": true}}),
        json!({"preview": {"loading": []}}),
        json!({"preview": {"loading": {"entry": ""}}}),
        json!({"preview": {"loading": {"entry": "../loading.html"}}}),
        json!({"preview": {"loading": {"entry": "./loading.html"}}}),
        json!({"preview": {"loading": {"entry": "/tmp/loading.html"}}}),
        json!({"preview": {"loading": {"entry": "C:/loading.html"}}}),
        json!({"preview": {"loading": {"entry": "https://example.com/loading.html"}}}),
        json!({"preview": {"loading": {"entry": "ui\\loading.html"}}}),
        json!({"preview": {"loading": {"entry": "ui/loading.html", "url": "https://example.com"}}}),
    ] {
        assert!(validate_extensions(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn hooks_and_search_validate_match_instead_of_ignoring_bad_rules() {
    for feature in ["hooks", "search"] {
        assert!(validate_extensions(&json!({feature:{"match":{"extensions":["zip"]}}})).is_ok());
        for predicate in [
            json!({"extensions":"zip"}),
            json!({"selection":"single"}),
            json!({"contains":[".git"]}),
            json!({"extension":["zip"]}),
        ] {
            assert!(validate_extensions(&json!({feature:{"match":predicate}})).is_err());
        }
    }
}

#[test]
fn metadata_and_file_operations_are_independent_grants() {
    let manifest: PluginManifest = serde_json::from_value(json!({
        "schemaVersion": 2,
        "id": "test.permissions", "name": "Permissions", "version": "1",
        "extensions": {},
        "permissions": {
            "metadata": {"enabled": true},
            "files": {
                "read": {"enabled": false},
                "create": {"enabled": true},
                "delete": {"enabled": true}
            }
        }
    }))
    .unwrap();
    let grants = declared_permissions(&manifest).unwrap();
    assert!(grants.contains("metadata"));
    assert!(grants.contains("files.create"));
    assert!(grants.contains("files.delete"));
    assert!(!grants.contains("files.read"));
    assert!(!grants.contains("folders.read"));
    assert!(serde_json::from_value::<PluginManifest>(json!({
        "schemaVersion": 2,
        "id": "test.legacy", "name": "Legacy", "version": "1",
        "contributions": {}
    }))
    .is_err());
}
