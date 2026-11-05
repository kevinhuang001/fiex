//! Builtin decorations share EventRouter and Collector with plugin providers.
use super::types::{Decoration, DecorationEntry, NativeBadge};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeKind {
    Tags,
    ConnectionStatus,
    ApplicationIcon,
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
    fiex_decorations::connection_badges(
        entry.sync.as_deref(),
        entry.uploaded,
        entry.has_cache,
        entry.pinned,
        |value| crate::i18n::t(value).to_string(),
    )
}
pub(super) fn resolve_in(
    kind: NativeKind,
    directory: &str,
    entries: &HashMap<String, DecorationEntry>,
) -> Vec<Decoration> {
    let application_info = if kind == NativeKind::ApplicationIcon {
        let paths = entries.keys().cloned().collect::<Vec<_>>();
        crate::core::volumes::applications::decoration_info(&paths)
    } else {
        HashMap::new()
    };
    let mut decorations = Vec::new();
    for entry in entries.values() {
        if kind == NativeKind::ApplicationIcon {
            let info = application_info.get(&entry.path);
            let recognized = entry.mime == "application/x-fiex-application"
                && info.is_some_and(|info| info.is_system);
            if directory != crate::core::volumes::applications::URI && info.is_none() {
                continue;
            }
            let mut decoration = Decoration {
                path: entry.path.clone(),
                native_badges: if recognized {
                    vec![badge("系统", "blue", None, None)]
                } else {
                    vec![]
                },
                ..Default::default()
            };
            decoration.icon_path = application_icon(entry, info);
            decorations.push(decoration);
            continue;
        }
        let native_badges = match kind {
            NativeKind::ConnectionStatus => connection(entry),
            NativeKind::Tags => fiex_decorations::tag_badges(
                entry
                    .tags
                    .iter()
                    .map(|tag| (tag.name.as_str(), tag.color, tag.hex.as_deref())),
            ),
            NativeKind::ApplicationIcon => unreachable!("application icon handled above"),
        };
        if !native_badges.is_empty() {
            decorations.push(Decoration {
                path: entry.path.clone(),
                native_badges,
                ..Default::default()
            });
        }
    }
    decorations
}

fn application_icon(
    entry: &DecorationEntry,
    info: Option<&fiex_applications::ApplicationDecorationInfo>,
) -> Option<String> {
    if let Some(source) = info.and_then(|info| info.icon_source.as_deref()) {
        return crate::core::volumes::applications::cached_icon(source);
    }
    (info.is_none() && entry.extension.eq_ignore_ascii_case("app"))
        .then(|| crate::core::volumes::applications::cached_icon(&entry.path))
        .flatten()
}
