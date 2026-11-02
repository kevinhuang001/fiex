//! Dynamic decorations: invoke a plugin method for a directory, sanitize the
//! response, and merge every provider's contribution into one result.
//!
//! Plugin output is untrusted: colors, text, badge counts and field names are
//! all validated here before the renderer sees them. Merging is priority
//! ordered so a higher-priority provider can override labels, styles and icons.

use super::types::{
    Decoration, DecorationBadge, DecorationEntry, DecorationField, DecorationStyle,
};
use crate::plugins::host::PluginManifest;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{DecorationContribution, ResourceRef};
use std::collections::HashMap;

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
        background_light: value.background_light.filter(|value| allowed_color(value)),
        background_dark: value.background_dark.filter(|value| allowed_color(value)),
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
        && style.background_light.is_none()
        && style.background_dark.is_none()
        && style.font_weight.is_none()
        && style.font_style.is_none()
}

fn merge_style(target: &mut DecorationStyle, source: &DecorationStyle) {
    if source.color.is_some() {
        target.color.clone_from(&source.color);
    }
    if source.background_light.is_some() {
        target.background_light.clone_from(&source.background_light);
    }
    if source.background_dark.is_some() {
        target.background_dark.clone_from(&source.background_dark);
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

pub(super) fn fields_with_name_contribution(
    mut fields: HashMap<String, DecorationField>,
    badges: Vec<DecorationBadge>,
    style: DecorationStyle,
) -> HashMap<String, DecorationField> {
    let badges = badges
        .into_iter()
        .filter_map(safe_badge)
        .take(8)
        .collect::<Vec<_>>();
    let style = safe_style(style);
    if !badges.is_empty() || !style_is_empty(&style) {
        let name = fields.entry("name".into()).or_default();
        name.badges.extend(badges);
        merge_style(&mut name.style, &style);
    }
    safe_fields(fields)
}

pub(super) struct Pending {
    priority: i32,
    pub(super) value: Decoration,
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
    let fields = safe_fields(value.fields);
    let label = safe_text(value.label.as_deref(), 120);
    let description = safe_text(value.description.as_deref(), 200);
    if fields.is_empty()
        && value.native_badges.is_empty()
        && label.is_none()
        && description.is_none()
        && value.icon.is_none()
        && value.icon_path.is_none()
    {
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
    pending.value.native_badges.extend(value.native_badges);
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
        if value.icon.is_some() || value.icon_path.is_some() {
            pending.value.icon = value.icon;
            pending.value.icon_path = value.icon_path;
        }
    }
    pending.priority = pending.priority.max(priority);
}

pub(super) fn path_for_target<'a>(
    target: &ResourceRef,
    entries: &HashMap<String, DecorationEntry>,
    resources: &'a [ResourceSnapshot],
) -> Option<&'a str> {
    let resource = resources
        .iter()
        .find(|resource| &resource.reference == target)?;
    entries
        .contains_key(&resource.display)
        .then_some(resource.display.as_str())
}

pub(super) fn decode(
    entries: &HashMap<String, DecorationEntry>,
    resources: &[crate::plugins::resource::resource_context::ResourceSnapshot],
    manifest: &PluginManifest,
    folder: &std::path::Path,
    contributions: Vec<DecorationContribution>,
) -> Result<Vec<Decoration>, String> {
    if contributions.len() > entries.len().max(1) * 8 {
        return Err("too many decoration contributions".into());
    }
    let mut result = HashMap::new();
    for item in contributions.into_iter().take(entries.len().max(1)) {
        if !item.native_badges.is_empty() || item.icon_path.is_some() {
            return Err("plugins cannot set Core-owned decoration fields".into());
        }
        let target = resources
            .iter()
            .find(|resource| resource.reference == item.target)
            .ok_or("decoration target is outside the scoped request")?;
        let path = target.display.clone();
        if !entries.contains_key(&path) || result.contains_key(&path) {
            return Err("invalid or duplicate decoration target".into());
        }
        let fields = fields_with_name_contribution(item.fields, item.badges, item.style);
        let (icon, icon_path) = item
            .icon
            .as_deref()
            .and_then(|reference| crate::plugins::resources::resolve(manifest, folder, reference))
            .map(|(namespace, path)| (Some(namespace), Some(path)))
            .unwrap_or((None, None));
        let value = Decoration {
            path: path.clone(),
            label: safe_text(item.label.as_deref(), 120),
            description: safe_text(item.description.as_deref(), 200),
            fields,
            icon,
            icon_path,
            providers: vec![],
            native_badges: vec![],
        };
        result.insert(path, value);
    }
    Ok(result.into_values().collect())
}

#[cfg(test)]
mod tests;
