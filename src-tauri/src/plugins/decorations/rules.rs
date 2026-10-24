//! Declarative decoration rules.
//!
//! A rule matches entries by extension and/or directory flag and is evaluated
//! by Core itself; no plugin process is started for declarative mode.

use super::dynamic::{merge, Pending};
use super::types::{
    Decoration, DecorationBadge, DecorationEntry, DecorationField, DecorationStyle,
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DecorationRule {
    #[serde(default, rename = "match")]
    matcher: DecorationMatcher,
    #[serde(default)]
    badge: Option<DecorationBadge>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    style: Option<DecorationStyle>,
    #[serde(default)]
    fields: HashMap<String, DecorationField>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DecorationMatcher {
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    is_dir: Option<bool>,
}

pub(super) fn apply_rules(
    result: &mut HashMap<String, Pending>,
    entries: &HashMap<String, DecorationEntry>,
    provider: &str,
    priority: i32,
    rules: &[DecorationRule],
) {
    for entry in entries.values() {
        for rule in rules {
            if let Some(is_dir) = rule.matcher.is_dir {
                if entry.is_dir != is_dir {
                    continue;
                }
            }
            if !rule.matcher.extensions.is_empty()
                && !rule
                    .matcher
                    .extensions
                    .iter()
                    .any(|extension| extension.eq_ignore_ascii_case(&entry.extension))
            {
                continue;
            }
            let value = Decoration {
                path: entry.path.clone(),
                badges: rule.badge.clone().into_iter().collect(),
                label: rule.label.clone(),
                description: rule.description.clone(),
                style: rule.style.clone().unwrap_or_default(),
                fields: rule.fields.clone(),
                icon: None,
                icon_path: None,
                providers: vec![],
            };
            merge(result, entries, &entry.path, priority, provider, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_rules, DecorationMatcher, DecorationRule};
    use crate::plugin_decorations::types::{DecorationBadge, DecorationEntry};
    use std::collections::HashMap;

    #[test]
    fn declarative_rules_apply_without_a_plugin_process() {
        let entries = vec![DecorationEntry {
            path: "/tmp/a.zip".into(),
            name: "a.zip".into(),
            is_dir: false,
            extension: "zip".into(),
            mime: String::new(),
        }];
        let map = entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<HashMap<_, _>>();
        let mut result = HashMap::new();
        apply_rules(
            &mut result,
            &map,
            "Archive",
            0,
            &[DecorationRule {
                matcher: DecorationMatcher {
                    extensions: vec!["zip".into()],
                    is_dir: Some(false),
                },
                badge: Some(DecorationBadge {
                    text: "ZIP".into(),
                    color: "neutral".into(),
                    tooltip: None,
                }),
                ..Default::default()
            }],
        );
        assert_eq!(result.len(), 1);
    }
}
