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
    let mut decorations = Vec::new();
    for entry in entries.values() {
        if kind == NativeKind::ApplicationIcon {
            let recognized = entry.mime == "application/x-fiex-application"
                && crate::core::volumes::applications::is_system_app(&entry.path);
            if directory != crate::core::volumes::applications::URI
                && !crate::core::volumes::applications::is_user(&entry.path)
                && !crate::core::volumes::applications::is_system_app(&entry.path)
            {
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
            decoration.icon_path = application_icon(entry);
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

fn application_icon(entry: &DecorationEntry) -> Option<String> {
    use crate::core::volumes::applications;
    if applications::is_user(&entry.path) {
        let path = applications::asset_path(&entry.path).ok()?;
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
