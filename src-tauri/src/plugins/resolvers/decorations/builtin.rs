//! Builtin decorations share EventRouter and Collector with plugin providers.
use super::types::{Decoration, DecorationEntry, NativeBadge};
use crate::plugins::events::Resolver;
use serde_json::{json, Value};
use std::collections::HashMap;

pub(super) fn providers() -> Vec<Resolver> {
    [
        "builtin:tags",
        "builtin:readonly",
        "builtin:connection-status",
        "builtin:application-icon",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, id)| Resolver::builtin(id, -12 + index as i64))
    .rev()
    .collect()
}
fn badge(text: &'static str, color: &str, status: Option<&str>, icon: Option<&str>) -> NativeBadge {
    let text = crate::i18n::t(text).to_string();
    NativeBadge {
        tooltip: text.clone(),
        text,
        color: color.into(),
        status: status.map(str::to_owned),
        icon: icon.map(str::to_owned),
        dot: false,
    }
}
fn connection(entry: &DecorationEntry) -> Vec<NativeBadge> {
    let Some(status) = entry.sync.as_deref() else {
        return vec![];
    };
    let value = match status {
        "synced" => badge("已缓存", "green", Some(status), Some("cloud-check")),
        "cloud" => badge("未缓存", "gray", Some(status), Some("cloud-download")),
        "pinned" => badge("始终保留在本地", "purple", Some(status), Some("pin")),
        "pending" => badge("等待上传", "orange", Some(status), Some("cloud-check")),
        "conflict" => badge("同步冲突", "red", Some(status), Some("cloud-check")),
        _ => return vec![],
    };
    let mut badges = vec![value];
    if entry.uploaded && (entry.has_cache || entry.pinned) {
        badges.push(badge(
            "已上传",
            "blue",
            Some("uploaded"),
            Some("cloud-upload"),
        ));
    }
    badges
}
pub(super) fn resolve_in(
    id: &str, directory: &str, entries: &HashMap<String, DecorationEntry>,
) -> Result<Value, String> {
    let mut decorations = Vec::new();
    for entry in entries.values() {
        if id == "builtin:application-icon" {
            let recognized = directory == crate::core::volumes::applications::URI && entry.mime == "application/x-fiex-application"
                && !crate::core::volumes::applications::is_user(&entry.path);
            if directory != crate::core::volumes::applications::URI && !crate::core::volumes::applications::is_user(&entry.path) { continue; }
            let mut decoration = Decoration {
                path: entry.path.clone(),
                native_badges: if recognized { vec![badge("系统", "blue", None, None)] } else { vec![] },
                ..Default::default()
            };
            decoration.icon_path = application_icon(entry);
            decorations.push(decoration);
            continue;
        }
        let native_badges = match id {
            "builtin:connection-status" => connection(entry),
            "builtin:readonly" if entry.readonly => vec![badge("只读", "gray", None, None)],
            "builtin:readonly" => vec![],
            "builtin:tags" => entry
                .tags
                .iter()
                .take(5)
                .map(|tag| {
                    const COLORS: &[&str] = &[
                        "#8e8e93", "#8e8e93", "#34c759", "#af52de", "#007aff", "#ffcc00",
                        "#ff3b30", "#ff9500",
                    ];
                    let color = tag
                        .hex
                        .as_deref()
                        .filter(|hex| {
                            hex.starts_with('#')
                                && matches!(hex.len(), 4 | 7)
                                && hex[1..].chars().all(|c| c.is_ascii_hexdigit())
                        })
                        .unwrap_or_else(|| {
                            COLORS.get(tag.color as usize).copied().unwrap_or(COLORS[0])
                        });
                    NativeBadge {
                        text: tag.name.clone(),
                        tooltip: tag.name.clone(),
                        color: color.into(),
                        dot: true,
                        ..Default::default()
                    }
                })
                .collect(),
            _ => return Err("unknown builtin decoration provider".into()),
        };
        if !native_badges.is_empty() {
            decorations.push(Decoration {
                path: entry.path.clone(),
                native_badges,
                ..Default::default()
            });
        }
    }
    Ok(json!({"decorations":decorations}))
}

fn application_icon(entry: &DecorationEntry) -> Option<String> {
    use crate::core::volumes::applications;
    if applications::is_user(&entry.path) {
        let path = applications::stored_path(&entry.path).ok()?;
        return applications::cached_icon(&path.to_string_lossy());
    }
    let source = if entry.mime == "application/x-fiex-application" {
        entry.link_target.as_deref()?
    } else if entry.extension.eq_ignore_ascii_case("app") {
        entry.path.as_str()
    } else {
        return None;
    };
    applications::cached_icon(source)
}

