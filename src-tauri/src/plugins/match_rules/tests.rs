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
