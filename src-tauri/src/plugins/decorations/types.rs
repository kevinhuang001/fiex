//! Decoration value types shared by declarative rules, dynamic plugin
//! responses and the renderer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    #[serde(default)]
    pub extension: String,
    #[serde(default)]
    pub mime: String,
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
