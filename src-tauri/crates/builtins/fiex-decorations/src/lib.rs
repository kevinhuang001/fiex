//! Shared Core-owned badge projection for filesystem metadata.
//!
//! Provider matching, request scope, application icons, and decoration
//! aggregation remain with their owning modules. This crate contains only the
//! reusable projection of tags and storage connection state into the shared
//! native badge DTO.

use fiex_provider_contracts::NativeDecorationBadge;

pub const MODULE_ID: &str = "core:decorations";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeKind {
    Tags,
    ConnectionStatus,
    ApplicationIcon,
}

mod providers;

pub use providers::{DecorationModule, DecorationModuleServices, DecorationProviderRegistration};

pub fn tag_badges<'a>(
    tags: impl IntoIterator<Item = (&'a str, u8, Option<&'a str>)>,
) -> Vec<NativeDecorationBadge> {
    const COLORS: &[&str] = &[
        "#8e8e93", "#8e8e93", "#34c759", "#af52de", "#007aff", "#ffcc00", "#ff3b30", "#ff9500",
    ];

    tags.into_iter()
        .take(5)
        .map(|(name, color, hex)| {
            let color = hex
                .filter(|value| is_valid_hex(value))
                .unwrap_or_else(|| COLORS.get(color as usize).copied().unwrap_or(COLORS[0]));
            NativeDecorationBadge {
                text: name.to_owned(),
                tooltip: name.to_owned(),
                color: color.into(),
                dot: true,
                ..Default::default()
            }
        })
        .collect()
}

pub fn connection_badges(
    status: Option<&str>,
    uploaded: bool,
    has_cache: bool,
    pinned: bool,
    localize: impl Fn(&'static str) -> String,
) -> Vec<NativeDecorationBadge> {
    let Some(status) = status else {
        return Vec::new();
    };
    let primary = match status {
        "synced" => badge(
            "已缓存",
            "green",
            Some(status),
            Some("cloud-check"),
            &localize,
        ),
        "cloud" => badge(
            "未缓存",
            "gray",
            Some(status),
            Some("cloud-download"),
            &localize,
        ),
        "pinned" => badge(
            "始终保留在本地",
            "purple",
            Some(status),
            Some("pin"),
            &localize,
        ),
        "pending" => badge(
            "等待上传",
            "orange",
            Some(status),
            Some("cloud-check"),
            &localize,
        ),
        "conflict" => badge(
            "同步冲突",
            "red",
            Some(status),
            Some("cloud-check"),
            &localize,
        ),
        _ => return Vec::new(),
    };
    let mut badges = vec![primary];
    if uploaded && (has_cache || pinned) {
        badges.push(badge(
            "已上传",
            "blue",
            Some("uploaded"),
            Some("cloud-upload"),
            &localize,
        ));
    }
    badges
}

fn badge(
    text: &'static str,
    color: &'static str,
    status: Option<&str>,
    icon: Option<&str>,
    localize: &impl Fn(&'static str) -> String,
) -> NativeDecorationBadge {
    let text = localize(text);
    NativeDecorationBadge {
        tooltip: text.clone(),
        text,
        color: color.into(),
        status: status.map(str::to_owned),
        icon: icon.map(str::to_owned),
        dot: false,
    }
}

fn is_valid_hex(value: &str) -> bool {
    value.starts_with('#')
        && matches!(value.len(), 4 | 7)
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(value: &'static str) -> String {
        value.to_owned()
    }

    #[test]
    fn tags_keep_only_five_and_reject_malformed_custom_colors() {
        let tags = [
            ("one", 2, Some("#abc")),
            ("two", 3, Some("red")),
            ("three", 4, None),
            ("four", 5, None),
            ("five", 6, None),
            ("six", 7, None),
        ];
        let badges = tag_badges(tags);
        assert_eq!(badges.len(), 5);
        assert_eq!(badges[0].color, "#abc");
        assert_eq!(badges[1].color, "#af52de");
        assert!(badges.iter().all(|badge| badge.dot));
    }

    #[test]
    fn connection_status_adds_uploaded_badge_only_when_locally_available() {
        let badges = connection_badges(Some("synced"), true, true, false, identity);
        assert_eq!(badges.len(), 2);
        assert_eq!(badges[0].status.as_deref(), Some("synced"));
        assert_eq!(badges[1].status.as_deref(), Some("uploaded"));
        assert_eq!(
            connection_badges(Some("cloud"), true, false, false, identity).len(),
            1
        );
        assert!(connection_badges(Some("unknown"), true, true, false, identity).is_empty());
    }
}
