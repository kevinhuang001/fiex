use super::{allowed_color, safe_badge, safe_fields};
use crate::plugins::resolvers::decorations::types::{
    DecorationBadge, DecorationField, DecorationStyle,
};
use std::collections::HashMap;

#[test]
fn rejects_unsafe_values() {
    assert!(allowed_color("#fff"));
    assert!(allowed_color("orange"));
    assert!(!allowed_color("red; background:url(x)"));
    assert!(safe_badge(DecorationBadge {
        text: "M".into(),
        color: "orange".into(),
        tooltip: None
    })
    .is_some());
    assert!(safe_badge(DecorationBadge {
        text: "TOOLONG".into(),
        color: "orange".into(),
        tooltip: None
    })
    .is_none());
}

#[test]
fn fields_are_sanitized_per_target() {
    let mut fields = HashMap::new();
    fields.insert(
        "modified".into(),
        DecorationField {
            badges: vec![DecorationBadge {
                text: "新".into(),
                color: "green".into(),
                tooltip: None,
            }],
            style: DecorationStyle {
                color: Some("#c2410c".into()),
                ..Default::default()
            },
        },
    );
    fields.insert(
        "row".into(),
        DecorationField {
            badges: vec![DecorationBadge {
                text: "R".into(),
                color: "red".into(),
                tooltip: None,
            }],
            style: DecorationStyle {
                background_light: Some("#fff9f0".into()),
                background_dark: Some("#2b2118".into()),
                ..Default::default()
            },
        },
    );
    fields.insert(
        "bogus".into(),
        DecorationField {
            style: DecorationStyle {
                color: Some("#000".into()),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let safe = safe_fields(fields);
    assert!(safe.contains_key("modified"));
    assert!(safe.contains_key("row"));
    assert!(!safe.contains_key("bogus"));
    assert!(safe["row"].badges.is_empty());
    assert_eq!(safe["modified"].style.color.as_deref(), Some("#c2410c"));
}
