//! Decoration value types shared by declarative rules, dynamic plugin
//! responses and the renderer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct DecorationEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    #[serde(default)]
    pub extension: String,
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub tags: Vec<DecorationTag>,
    #[serde(default)]
    pub sync: Option<String>,
    #[serde(default)]
    pub has_cache: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub uploaded: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DecorationTag {
    pub name: String,
    pub color: u8,
    #[serde(default)]
    pub hex: Option<String>,
}

/// Presentation metadata emitted only by Core builtin providers. Plugin
/// decoding never accepts this field, so plugins cannot forge native status.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct NativeBadge {
    pub text: String,
    pub color: String,
    pub tooltip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub dot: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationBadge {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub tooltip: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationStyle {
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub font_weight: Option<String>,
    #[serde(default)]
    pub font_style: Option<String>,
    #[serde(default)]
    pub background_light: Option<String>,
    #[serde(default)]
    pub background_dark: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationField {
    #[serde(default)]
    pub badges: Vec<DecorationBadge>,
    #[serde(default)]
    pub style: DecorationStyle,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decoration {
    #[serde(default)]
    pub native_badges: Vec<NativeBadge>,
    pub path: String,
    #[serde(default)]
    pub badges: Vec<DecorationBadge>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub style: DecorationStyle,
    /// Per-column decorations keyed by `name`, `modified`, `size`, `kind`
    /// or `row`. `row` only accepts `style`.
    #[serde(default)]
    pub fields: HashMap<String, DecorationField>,
    /// Namespaced reference (`fiex-resource://<plugin-id>/<resource-id>`).
    #[serde(default)]
    pub icon: Option<String>,
    /// Resolved absolute path for the renderer's asset protocol.
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub providers: Vec<String>,
}
