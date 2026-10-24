//! Dynamic decorations: invoke a plugin method for a directory, sanitize the
//! response, and merge every provider's contribution into one result.
//!
//! Plugin output is untrusted: colors, text, badge counts and field names are
//! all validated here before the renderer sees them. Merging is priority
//! ordered so a higher-priority provider can override labels, styles and icons.

use super::rules::{apply_rules, DecorationRule};
use super::types::{
    Decoration, DecorationBadge, DecorationEntry, DecorationField, DecorationStyle,
};
use crate::plugin_host::{self, PluginManifest};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecorationConfig {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    scope: Option<DecorationScope>,
    #[serde(default)]
    rules: Vec<DecorationRule>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecorationScope {
    #[serde(default)]
    marker: Option<String>,
}

fn config(manifest: &PluginManifest) -> DecorationConfig {
    manifest
        .permissions
        .folders
        .read
        .decorations
        .as_ref()
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

fn allowed_color(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    matches!(
        value.as_str(),
        "orange" | "green" | "red" | "blue" | "gray" | "neutral" | "purple"
    ) || (value.starts_with('#')
        && matches!(value.len(), 4 | 7)
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit()))
}

fn safe_text(value: Option<&str>, limit: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.chars().count() <= limit)
        .map(str::to_owned)
}

fn safe_badge(value: DecorationBadge) -> Option<DecorationBadge> {
    let text = value.text.trim().to_owned();
    if text.is_empty() || text.chars().count() > 4 {
        return None;
    }
    let color = if allowed_color(&value.color) {
        value.color.trim().to_ascii_lowercase()
    } else {
        "neutral".into()
    };
    Some(DecorationBadge {
        text,
        color,
        tooltip: safe_text(value.tooltip.as_deref(), 200),
    })
}

fn safe_style(value: DecorationStyle) -> DecorationStyle {
    DecorationStyle {
        color: value.color.filter(|value| allowed_color(value)),
        background: value.background.filter(|value| allowed_color(value)),
        font_weight: value
            .font_weight
            .filter(|value| matches!(value.as_str(), "normal" | "500" | "600" | "bold")),
        font_style: value
            .font_style
            .filter(|value| matches!(value.as_str(), "normal" | "italic")),
    }
}

fn style_is_empty(style: &DecorationStyle) -> bool {
    style.color.is_none()
        && style.background.is_none()
        && style.font_weight.is_none()
        && style.font_style.is_none()
}

fn merge_style(target: &mut DecorationStyle, source: &DecorationStyle) {
    if source.color.is_some() {
        target.color.clone_from(&source.color);
    }
    if source.background.is_some() {
        target.background.clone_from(&source.background);
    }
    if source.font_weight.is_some() {
        target.font_weight.clone_from(&source.font_weight);
    }
    if source.font_style.is_some() {
        target.font_style.clone_from(&source.font_style);
    }
}

fn safe_field(value: DecorationField) -> Option<DecorationField> {
    let badges = value
        .badges
        .into_iter()
        .filter_map(safe_badge)
        .take(3)
        .collect::<Vec<_>>();
    let style = safe_style(value.style);
    if badges.is_empty() && style_is_empty(&style) {
        None
    } else {
        Some(DecorationField { badges, style })
    }
}

fn safe_fields(fields: HashMap<String, DecorationField>) -> HashMap<String, DecorationField> {
    fields
        .into_iter()
        .filter(|(key, _)| matches!(key.as_str(), "name" | "modified" | "size" | "kind" | "row"))
        .filter_map(|(key, mut field)| {
            // A row is not a value cell; only its style is meaningful.
            if key == "row" {
                field.badges.clear();
            }
            safe_field(field).map(|field| (key, field))
        })
        .collect()
}

fn marker_matches(directory: &str, marker: &str) -> bool {
    if marker.is_empty() || marker.contains(['/', '\\']) {
        return false;
    }
    let mut current = std::path::Path::new(directory);
    loop {
        if current.join(marker).exists() {
            return true;
        }
        let Some(parent) = current.parent() else {
            return false;
        };
        current = parent;
    }
}

pub(super) struct Pending {
    priority: i32,
    value: Decoration,
}

pub(super) fn merge(
    result: &mut HashMap<String, Pending>,
    entries: &HashMap<String, DecorationEntry>,
    path: &str,
    priority: i32,
    provider: &str,
    value: Decoration,
) {
    if !entries.contains_key(path) {
        return;
    }
    let mut fields = value.fields;
    // Legacy `badge` / `badges` / `style` keep their original meaning: the
    // file-name cell.
    if !value.badges.is_empty() || !style_is_empty(&value.style) {
        let name = fields.entry("name".into()).or_default();
        name.badges.extend(value.badges);
        merge_style(&mut name.style, &value.style);
    }
    let fields = safe_fields(fields);
    let label = safe_text(value.label.as_deref(), 120);
    let description = safe_text(value.description.as_deref(), 200);
    if fields.is_empty() && label.is_none() && description.is_none() && value.icon.is_none() {
        return;
    }
    let pending = result.entry(path.to_owned()).or_insert_with(|| Pending {
        priority,
        value: Decoration {
            path: path.to_owned(),
            ..Default::default()
        },
    });
    if !pending.value.providers.iter().any(|item| item == provider) {
        pending.value.providers.push(provider.to_owned());
    }
    for (key, field) in fields {
        let target = pending.value.fields.entry(key).or_default();
        for badge in field.badges {
            if target.badges.len() < 3 && !target.badges.iter().any(|item| item.text == badge.text)
            {
                target.badges.push(badge);
            }
        }
        if priority >= pending.priority {
            merge_style(&mut target.style, &field.style);
        }
    }
    if priority >= pending.priority {
        if let Some(label) = label {
            pending.value.label = Some(label);
        }
        if let Some(description) = description {
            pending.value.description = Some(description);
        }
        if value.icon.is_some() {
            pending.value.icon = value.icon;
            pending.value.icon_path = value.icon_path;
        }
    }
    pending.priority = pending.priority.max(priority);
}

fn dynamic(
    result: &mut HashMap<String, Pending>,
    entries: &HashMap<String, DecorationEntry>,
    manifest: &PluginManifest,
    folder: &std::path::Path,
    directory: &str,
    config: &DecorationConfig,
) {
    let Some(method) = config.method.as_deref().filter(|value| !value.is_empty()) else {
        return;
    };
    if let Some(scope) = &config.scope {
        if let Some(marker) = &scope.marker {
            if !marker_matches(directory, marker) {
                return;
            }
        }
    }
    let declared = plugin_host::declared_permissions(manifest).unwrap_or_default();
    if !crate::plugin_permissions::allowed(&manifest.id, &declared, "folders.read").unwrap_or(false)
    {
        return;
    }
    let payload = json!({
        "directory": directory,
        "entries": entries.values().collect::<Vec<_>>(),
    });
    let Ok(response) = plugin_host::global().invoke_at(&manifest.id, method, payload, directory)
    else {
        return;
    };
    let Some(items) = response.get("decorations").and_then(Value::as_array) else {
        return;
    };
    for item in items.iter().take(entries.len().max(1)) {
        let Ok(path) = item.get("path").and_then(Value::as_str).ok_or(()) else {
            continue;
        };
        let mut badges = Vec::new();
        if let Some(badge) = item.get("badge").cloned() {
            if let Ok(badge) = serde_json::from_value::<DecorationBadge>(badge) {
                badges.push(badge);
            }
        }
        if let Some(values) = item.get("badges").and_then(Value::as_array) {
            badges.extend(
                values
                    .iter()
                    .cloned()
                    .filter_map(|value| serde_json::from_value(value).ok()),
            );
        }
        let style = item
            .get("style")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        let mut fields = HashMap::new();
        if let Some(values) = item.get("fields").and_then(Value::as_object) {
            for (key, value) in values {
                if let Ok(field) = serde_json::from_value::<DecorationField>(value.clone()) {
                    fields.insert(key.clone(), field);
                }
            }
        }
        let (icon, icon_path) = item
            .get("icon")
            .and_then(Value::as_str)
            .and_then(|reference| crate::plugin_resources::resolve(manifest, folder, reference))
            .map(|(namespace, path)| (Some(namespace), Some(path)))
            .unwrap_or((None, None));
        let value = Decoration {
            path: path.into(),
            badges,
            label: item.get("label").and_then(Value::as_str).map(str::to_owned),
            description: item
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
            style,
            fields,
            icon,
            icon_path,
            providers: vec![],
        };
        merge(
            result,
            entries,
            path,
            config.priority,
            &manifest.name,
            value,
        );
    }
}

pub fn query(directory: &str, entries: Vec<DecorationEntry>) -> Vec<Decoration> {
    let entries = entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<HashMap<_, _>>();
    let mut result = HashMap::new();
    for (manifest, folder) in plugin_host::installed_manifests() {
        let config = config(&manifest);
        if config.mode == "none" || config.mode.is_empty() {
            continue;
        }
        let declared = plugin_host::declared_permissions(&manifest).unwrap_or_default();
        if !crate::plugin_permissions::allowed(&manifest.id, &declared, "folders.read")
            .unwrap_or(false)
        {
            continue;
        }
        match config.mode.as_str() {
            "declarative" => apply_rules(
                &mut result,
                &entries,
                &manifest.name,
                config.priority,
                &config.rules,
            ),
            "dynamic" => dynamic(
                &mut result,
                &entries,
                &manifest,
                &folder,
                directory,
                &config,
            ),
            _ => {}
        }
    }
    let mut values = result
        .into_values()
        .map(|pending| pending.value)
        .collect::<Vec<_>>();
    values.sort_by(|a, b| a.path.cmp(&b.path));
    values
}

#[cfg(test)]
mod tests {
    use super::{allowed_color, safe_badge, safe_fields};
    use crate::plugin_decorations::types::{DecorationBadge, DecorationField, DecorationStyle};
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
                    background: Some("#fff9f0".into()),
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
}
