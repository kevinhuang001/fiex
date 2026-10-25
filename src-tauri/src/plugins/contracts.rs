//! Versioned standard-event DTOs shared by Core, the SDK and plugins.
//!
//! These types are the wire contract behind `info.resolve` and
//! `preview.open`. Core builds a context after permission projection; plugins
//! return the response types. No local filesystem path is part of either shape.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INFO_CONTEXT_VERSION: u32 = 1;
pub const PREVIEW_CONTEXT_VERSION: u32 = 1;

/// Opaque resource identity owned by Core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    pub mount_id: String,
    pub node_id: String,
}

/// The subset of resource attributes a plugin may receive.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetadata {
    pub name: String,
    /// `file` or `directory`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<i64>,
}

/// A scoped read capability. The opaque id can be opened with
/// `host.fs.openResource`; it never contains a host path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedRead {
    pub resource_id: String,
}

/// Standard resource context for Info, Preview and Decoration events.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContext {
    #[serde(rename = "ref")]
    pub reference: ResourceRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ResourceMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<ScopedRead>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoContext {
    pub version: u32,
    pub session_id: String,
    #[serde(rename = "intent")]
    pub intent: String,
    pub resource: ResourceContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoRow {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InfoSectionView {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub rows: Vec<InfoRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InfoResponse {
    #[serde(default)]
    pub sections: Vec<InfoSectionView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewContext {
    pub version: u32,
    pub session_id: String,
    #[serde(rename = "intent")]
    pub intent: String,
    pub resource: ResourceContext,
}

/// A package-confined HTML preview entry returned by a provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageView {
    /// Only `package` is accepted.
    pub source: String,
    /// Single-component-safe relative path inside the plugin package.
    pub entry: String,
}

/// Standard preview result. `kind` is a closed, versioned enum:
/// `html` (package-confined provider UI), `text` (bounded inline text), or
/// `unsupported` (explicit fallback).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewView {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<PackageView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub state: Value,
}

impl PreviewView {
    pub fn html(entry: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            kind: "html".into(),
            title: Some(title.into()),
            view: Some(PackageView {
                source: "package".into(),
                entry: entry.into(),
            }),
            text: None,
            message: None,
            state: Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn context_uses_opaque_ref_and_optional_projection() {
        let context = InfoContext {
            version: INFO_CONTEXT_VERSION,
            session_id: "s1".into(),
            intent: "info".into(),
            resource: ResourceContext {
                reference: ResourceRef {
                    mount_id: "local".into(),
                    node_id: "fiex-resource:opaque".into(),
                },
                metadata: None,
                read: None,
            },
        };
        let value = serde_json::to_value(&context).unwrap();
        assert_eq!(value["resource"]["ref"]["nodeId"], "fiex-resource:opaque");
        assert!(value["resource"].get("metadata").is_none());
        assert!(value["resource"].get("read").is_none());
    }

    #[test]
    fn preview_kind_is_closed_and_package_only() {
        let value = json!({"kind":"html","view":{"source":"package","entry":"ui/preview.html"}});
        let view: PreviewView = serde_json::from_value(value).unwrap();
        assert_eq!(view.kind, "html");
        assert_eq!(view.view.unwrap().source, "package");
        assert!(
            serde_json::from_value::<PreviewView>(json!({"kind":"iframe","url":"https://x"}))
                .is_err()
        );
    }

    #[test]
    fn info_response_has_stable_section_ids() {
        let response: InfoResponse = serde_json::from_value(json!({
            "sections":[{"id":"core.metadata","title":"文件","rows":[{"label":"大小","value":"1 KiB"}]}]
        }))
        .unwrap();
        assert_eq!(response.sections[0].id, "core.metadata");
    }
}
