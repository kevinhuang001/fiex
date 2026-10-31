use super::*;
use serde_json::json;

#[test]
fn absent_context_does_not_bypass_rules() {
    for predicate in [
        json!({"extensions":["zip"]}),
        json!({"mimeTypes":["text/*"]}),
        json!({"kinds":["file"]}),
        json!({"selection":"single"}),
    ] {
        let rules = MatchRules::from_contribution(&json!({"match":predicate})).unwrap();
        assert!(!rules.matches_event(&[], ""), "{predicate}");
    }
    let rules = MatchRules::from_contribution(&json!({"match":{"kinds":["file"]}})).unwrap();
    assert!(!rules.matches_event(&[Value::Null], ""));
    assert!(!rules.matches_event(&[json!({"extension":"txt"})], ""));
    for contribution in [json!(true), json!({}), json!({"match":{}})] {
        assert!(MatchRules::from_contribution(&contribution)
            .unwrap()
            .matches_event(&[], ""));
    }
}

#[test]
fn source_and_directory_predicates_are_rejected() {
    for key in [
        "contains",
        "schemes",
        "providerIds",
        "mountIds",
        "location",
        "onlyOnVfs",
    ] {
        assert!(MatchRules::from_contribution(&json!({"match":{key:["local"]}})).is_err());
    }
}

#[test]
fn mixed_selections_require_every_target_and_operation() {
    let rules =
        MatchRules::from_contribution(&json!({"match":{"kinds":["file"],"requiredOps":["read"]}}))
            .unwrap();
    assert!(rules.matches_event(
        &[json!({"kind":"file","operations":["stat","read"]})],
        "recent://"
    ));
    assert!(!rules.matches_event(&[json!({"kind":"file","operations":["stat"]})], "/tmp"));
    assert!(!rules.matches_event(
        &[
            json!({"kind":"file","operations":["read"]}),
            json!({"kind":"directory","operations":["read"]})
        ],
        "/tmp"
    ));
}

#[test]
fn malformed_rules_fail_closed() {
    for rules in [
        json!({"extensions":"zip"}),
        json!({"extensions":[1]}),
        json!({"mimeType":["text/*"]}),
        json!(null),
    ] {
        let contribution = json!({"match":rules});
        assert!(MatchRules::from_contribution(&contribution).is_err());
        assert!(validate(contribution.get("match"), false, false).is_err());
    }
}

#[test]
fn match_is_an_include_predicate() {
    let rules = MatchRules::from_contribution(&json!({
        "match": {"extensions":["md"],"mimeTypes":["text/*"],"kinds":["file"],"selection":"single"}
    }))
    .unwrap();
    assert!(rules.matches("md", "text/markdown", false));
    assert!(!rules.matches("png", "image/png", false));
    assert!(!rules.matches("md", "text/markdown", true));
}

#[test]
fn selection_and_content_are_scoped_predicates() {
    let decorations = MatchRules::from_contribution(&json!({
        "match": {"extensions": ["ts"]}
    }))
    .unwrap();
    assert!(decorations.matches("ts", "text/plain", false));
    assert!(!decorations.matches("md", "text/markdown", false));
    let menu = MatchRules::from_contribution(&json!({
        "match": {"selection": "single", "extensions": ["zip"]}
    }))
    .unwrap();
    assert!(menu.matches_selection(1));
    assert!(!menu.matches_selection(2));
    let blank: crate::plugins::match_rules::MatchRules =
        serde_json::from_value(json!({"selection":"zero"})).unwrap();
    assert!(blank.matches_selection(0));
    assert!(!blank.matches_selection(1));
    assert!(menu.matches("zip", "application/zip", false));
}

#[test]
fn match_object_and_wildcards_work() {
    let rules =
        MatchRules::from_contribution(&json!({"match":{"extensions":["*.ZIP","rar"]}})).unwrap();
    assert!(rules.matches("zip", "", false));
    assert!(rules.matches("rar", "", false));
    assert!(!rules.matches("7z", "", false));
    let all = MatchRules::from_contribution(&json!({"match":{"mimeTypes":["text/*"]}})).unwrap();
    assert!(all.matches("anything", "text/plain", false));
}

#[test]
fn view_and_storage_are_independent_and_match_all_selected_resources() {
    let rules = MatchRules::from_contribution(&json!({"match":{
        "view":{"providerIds":["core:recent"]},
        "storage":{"types":["vfs"],"providerIds":["plugin:org.example.drive:gdrive"]},
        "kinds":["file"],"requiredOps":["read"]
    }})).unwrap();
    let remote = json!({
        "view":{"type":"vfs","providerId":"core:recent","mountId":"core:recent"},
        "storage":{"type":"vfs","providerId":"plugin:org.example.drive:gdrive","mountId":"account-1"},
        "kind":"file","operations":["read"]
    });
    assert!(rules.matches_event(&[remote.clone()], ""));
    let missing_source = json!({"view":remote["view"],"kind":"file","operations":["read"]});
    assert!(!rules.matches_event(&[missing_source], ""));
    let another_account = json!({
        "view":remote["view"],
        "storage":{"type":"vfs","providerId":"plugin:org.example.drive:gdrive","mountId":"account-2"},
        "kind":"file","operations":["read"]
    });
    let mount_specific = MatchRules::from_contribution(&json!({"match":{
        "storage":{"mountIds":["account-1"]}
    }})).unwrap();
    assert!(mount_specific.matches_event(&[remote.clone()], ""));
    assert!(!mount_specific.matches_event(&[another_account.clone()], ""));
    assert!(!mount_specific.matches_event(&[remote, another_account], ""));
}

#[test]
fn location_dimensions_are_conjunctive_and_kind_is_not_resource_type() {
    let rule = MatchRules::from_contribution(&json!({"match":{
        "view":{"types":["vfs"],"providerIds":["core:applications"]},
        "kinds":["file"]
    }})).unwrap();
    assert!(rule.matches_event(&[json!({
        "view":{"type":"vfs","providerId":"core:applications","mountId":"core:applications"},
        "kind":"file"
    })], ""));
    assert!(!rule.matches_event(&[json!({
        "view":{"type":"fs","providerId":"core:applications","mountId":"core:applications"},
        "kind":"file"
    })], ""));
    assert!(!rule.matches_event(&[json!({
        "view":{"type":"vfs","providerId":"core:applications","mountId":"core:applications"},
        "kind":"directory"
    })], ""));
}

#[test]
fn invalid_fs_vfs_matches_fail_manifest_validation() {
    for predicate in [
        json!({"view":{"types":[]}}),
        json!({"storage":{"providerIds":[]}}),
        json!({"view":{"types":["remote"]}}),
        json!({"view":{"types":"vfs"}}),
        json!({"view":{"providerIds":[""]}}),
        json!({"storage":{}}),
        json!({"view":{"domains":["vfs"]}}),
        json!({"kinds":["vfs"]}),
        json!({"kinds":["unknown"]}),
    ] {
        assert!(validate(Some(&predicate), false, true).is_err(), "{predicate}");
    }
    for predicate in [
        json!({"view":{"types":["fs","vfs"]}}),
        json!({"storage":{"providerIds":["core:local-fs"]}}),
        json!({"view":{"mountIds":["account-1"]}}),
    ] {
        assert!(validate(Some(&predicate), false, true).is_ok(), "{predicate}");
    }
}

#[test]
fn blank_canvas_uses_trusted_directory_owner() {
    let root = crate::core::volumes::applications::URI;
    let rule = MatchRules::from_contribution(&json!({"match":{
        "view":{"providerIds":["core:applications"]},
        "selection":"zero","kinds":["directory"]
    }})).unwrap();
    assert!(rule.matches_event(&[], root));
    assert!(!rule.matches_event(&[], "/tmp"));
    assert!(!rule.matches_event(&[json!({"kind":"file"})], root));
}
