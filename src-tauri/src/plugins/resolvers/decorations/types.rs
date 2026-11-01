//! Decoration value types shared by declarative rules, dynamic plugin
//! responses and the renderer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use fiex_provider_contracts::{DecorationBadge, DecorationField, DecorationStyle};
pub type NativeBadge = fiex_provider_contracts::NativeDecorationBadge;

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
    pub allowed_ops: Vec<String>,
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
    /// The real bundle/shortcut a virtual application entry points at.
    #[serde(default)]
    pub link_target: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DecorationTag {
    pub name: String,
    pub color: u8,
    #[serde(default)]
    pub hex: Option<String>,
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
