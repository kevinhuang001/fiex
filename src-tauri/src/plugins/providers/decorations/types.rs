//! Decoration value types shared by declarative rules and the renderer.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use fiex_provider_contracts::{DecorationBadge, DecorationField, DecorationStyle};
pub type NativeBadge = fiex_provider_contracts::NativeDecorationBadge;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    #[serde(default)]
    pub extension: String,
    #[serde(default)]
    pub mime: String,
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

/// Core presentation result. Plugins contribute through the shared typed DTO;
/// this renderer shape adds only Core-owned paths, provenance and native badges.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decoration {
    #[serde(default)]
    pub native_badges: Vec<NativeBadge>,
    pub path: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub fields: HashMap<String, DecorationField>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub providers: Vec<String>,
}
