use super::*;
use serde_json::json;

#[test]
fn absent_context_does_not_bypass_rules() {
    for predicate in [
        json!({"extensions":["zip"]}),
        json!({"mimeTypes":["text/*"]}),
        json!({"kinds":["file"]}),
        json!({"selection":"single"}),
        json!({"contains":[".git"]}),
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
fn directory_markers_require_a_real_local_context() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    let child = dir.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let rules = MatchRules::from_contribution(&json!({"match":{"contains":[".git"]}})).unwrap();
    assert!(rules.matches_event(&[], child.to_str().unwrap()));
    for directory in ["", ".", "fiex://mount/root", "fiex:mount/root"] {
        assert!(!rules.matches_event(&[], directory), "{directory}");
    }
    let outside = tempfile::tempdir().unwrap();
    assert!(!rules.matches_event(&[], outside.path().to_str().unwrap()));
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
fn contains_and_selection_are_scoped_predicates() {
    let decorations = MatchRules::from_contribution(&json!({
        "match": {"contains": [".git"], "extensions": ["ts"]}
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
