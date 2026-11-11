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
    let info_subsections = json!([{"id":"text","fields":[
        {"id":"format","label":"Format","compute":"onOpen"}
    ]}]);
    assert!(
        validate_extensions(&serde_json::json!({
            "decorations": {"match": {"contains": [".git"]}}
        }))
        .is_err()
    );
    assert!(
        validate_extensions(&serde_json::json!({
            "contextMenus": {"match": {"selection": "single"}}
        }))
        .is_ok()
    );
    assert!(
        validate_extensions(&serde_json::json!({
            "contextMenus": {"match": {
                "exclude": {"view": {"providerIds": ["core:applications"]}}
            }}
        }))
        .is_ok()
    );
    assert!(
        validate_extensions(&serde_json::json!({
            "info": {"title":"Text","match": {"mimeTypes": ["text/*"]}, "subsections": info_subsections.clone()}
        }))
        .is_ok()
    );
    for invalid in [
        serde_json::json!({"decorations": {"match": {"selection": "single"}}}),
        serde_json::json!({"contextMenus": {"match": {"contains": [".git"]}}}),
        serde_json::json!({"info": {"title":"Text","match": {"exclude": {}}, "subsections": info_subsections.clone()}}),
        serde_json::json!({"info": {"title":"Text","match": {"contains": [".git"]}, "subsections": info_subsections.clone()}}),
        serde_json::json!({"info": [{"id": "x"}]}),
        serde_json::json!({"info": {"title": "X"}}),
        serde_json::json!({"info": {"id": "x"}}),
        serde_json::json!({"contextMenus": {"match": {"selection": "many"}}}),
    ] {
        assert!(validate_extensions(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn info_fields_are_static_strict_and_bounded() {
    let field = json!({"id":"format","label":"Format","compute":"onDemand"});
    let valid =
        json!({"info":{"title":"Archive","subsections":[{"id":"archive","fields":[field]}]}});
    assert!(validate_extensions(&valid).is_ok());

    let named = json!({"info":{"providers":[
        {"id":"metadata","title":"Metadata","subsections":[{"id":"basic","fields":[field]}],"match":{},"priority":100},
        {"id":"checksum","title":"Checksum","subsections":[{"id":"checksums","fields":[field]}],"match":{"kinds":["file"]},"priority":50}
    ]}});
    assert!(validate_extensions(&named).is_ok());

    for invalid in [
        json!({"info":{"match":{"extensions":["zip"]}}}),
        json!({"info":{"sections":[{"id":"archive","title":"Archive","fields":[
            {"id":"format","label":"Format","compute":"onDemand"}
        ]}]}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"archive","title":"Archive","fields":[
            {"id":"format","label":"Format","compute":"onDemand","cost":"high"}
        ]}]}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"archive","fields":[
            {"id":"format","label":"Format","compute":"onDemand","valueType":"text"}
        ]}]}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"archive","fields":[
            {"id":"format","label":"Format","compute":"onDemand"}
        ]}],"unknown":true}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"archive","fields":[
            {"id":"format","label":"Format","compute":"onDemand"}
        ]}],"priority":"high"}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"archive","fields":[
            {"id":"format","label":"Format","compute":"onClick"}
        ]}]}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"archive","fields":[
            {"id":"format","label":"Format"}
        ]}]}}),
        json!({"info":{"title":"Archive","subsections":[
            {"id":"archive","fields":[{"id":"format","label":"Format","compute":"onOpen"}]},
            {"id":"archive","fields":[{"id":"other","label":"Other","compute":"onOpen"}]}
        ]}}),
        json!({"info":{"providers":[{"id":"metadata","title":"Metadata","subsections":[{"id":"basic","fields":[{"id":"name","label":"Name","compute":"onOpen"}]}]}],"subsections":[{"id":"fallback","fields":[{"id":"x","label":"X","compute":"onOpen"}]}]}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"audio","match":{"mimeTypes":["*/mpeg"]},"fields":[{"id":"x","label":"X","compute":"onOpen"}]}]}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"audio","match":{"kinds":["vfs"]},"fields":[{"id":"x","label":"X","compute":"onOpen"}]}]}}),
        json!({"info":{"title":"Archive","subsections":[{"id":"audio","match":{"extensions":["MP3",".mp3"]},"fields":[{"id":"x","label":"X","compute":"onOpen"}]}]}}),
        json!({"info":{"providers":[{"id":"metadata","title":"Metadata","subsections":[{"id":"basic","fields":[{"id":"name","label":"Name","compute":"onOpen"}]}],"priority":100}]}}),
    ] {
        assert!(validate_extensions(&invalid).is_err(), "{invalid}");
    }

    let too_many_fields = (0..65)
        .map(|index| json!({"id":format!("field{index}"),"label":"Field","compute":"onDemand"}))
        .collect::<Vec<_>>();
    assert!(
        validate_extensions(&json!({"info":{"title":"Large","subsections":[{
            "id":"large","fields":too_many_fields
        }]}}))
        .is_err()
    );
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
    assert!(
        validate_extensions(&json!({"hooks":{
            "event":"afterOpen",
            "match":{"extensions":["zip"]}
        }}))
        .is_ok()
    );
    assert!(validate_extensions(&json!({"search":{"match":{"extensions":["zip"]}}})).is_ok());
    for (feature, required) in [
        ("hooks", json!({"event":"afterOpen"})),
        ("search", json!({})),
    ] {
        for predicate in [
            json!({"extensions":"zip"}),
            json!({"selection":"single"}),
            json!({"contains":[".git"]}),
            json!({"extension":["zip"]}),
        ] {
            let mut value = required.clone();
            value["match"] = predicate;
            assert!(validate_extensions(&json!({feature:value})).is_err());
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
    assert!(
        serde_json::from_value::<PluginManifest>(json!({
            "schemaVersion": 2,
            "id": "test.invalid", "name": "Invalid", "version": "1",
            "contributions": {}
        }))
        .is_err()
    );
}
