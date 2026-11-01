//! The same typed predicate gates discovery and scope preparation before native IPC.
use super::{event, ResourceSnapshot};
use fiex_provider_contracts::{FsOperation, MatchResource, ResourceCapabilities};
use serde_json::{json, Value};

pub(crate) fn matches(
    method: &str,
    extensions: &Value,
    targets: &[MatchResource],
    directory: &str,
) -> bool {
    let Some(contribution) = event(method).and_then(|event| extensions.get(event.extension)) else {
        return false;
    };
    if !contribution.is_object() && contribution.as_bool() != Some(true) {
        return false;
    }
    let Ok(rules) = crate::plugins::match_rules::MatchRules::from_contribution(contribution) else {
        return false;
    };
    // The blank canvas is selection 0. A plugin must opt into it explicitly;
    // "any" and the default predicate keep meaning a non-empty selection.
    if method == "contextMenu.resolve"
        && targets.is_empty()
        && rules.selection.as_deref() != Some("zero")
    {
        return false;
    }
    rules.matches_resources(targets, directory)
}

pub(super) fn matches_resources(
    method: &str,
    extensions: &Value,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
) -> bool {
    matches(
        method,
        extensions,
        &match_resources(resources),
        directory.unwrap_or(""),
    )
}

/// Build matcher input from Core's resolved resource and capability registries.
pub fn match_resources(resources: &[ResourceSnapshot]) -> Vec<MatchResource> {
    resources
        .iter()
        .map(|resource| {
            let (view, storage) =
                crate::plugins::match_rules::typed_locations_for(&resource.display);
            let effective = resource.operations.unwrap_or_else(|| {
                crate::plugins::resource::file_capabilities::for_path(&resource.display)
            });
            MatchResource {
                metadata: resource.metadata().cloned(),
                view,
                storage,
                capabilities: ResourceCapabilities {
                    operations: FsOperation::ALL
                        .into_iter()
                        .filter(|operation| effective.allows(*operation))
                        .collect(),
                },
            }
        })
        .collect()
}

/// Protocol projection. Provider metadata remains storage-specific JSON, but
/// candidate matching never consumes this value.
pub fn metadata(resources: &[ResourceSnapshot]) -> Vec<Value> {
    match_resources(resources)
        .into_iter()
        .map(|target| {
            let mut value = target
                .metadata
                .map(|metadata| serde_json::to_value(metadata).unwrap_or_else(|_| json!({})))
                .filter(Value::is_object)
                .unwrap_or_else(|| json!({}));
            value["operations"] = json!(target
                .capabilities
                .operations
                .iter()
                .map(|operation| operation.as_str())
                .collect::<Vec<_>>());
            if let Some(view) = target.view {
                value["view"] = serde_json::to_value(view).unwrap_or(Value::Null);
            }
            if let Some(storage) = target.storage {
                value["storage"] = serde_json::to_value(storage).unwrap_or(Value::Null);
            }
            value
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fiex_provider_contracts::{ResourceLocation, ResourceMetadata, ResourceType};

    fn target(kind: &str, extension: &str, view: Option<ResourceLocation>) -> MatchResource {
        MatchResource {
            metadata: Some(ResourceMetadata {
                kind: kind.into(),
                extension: Some(extension.into()),
                mime: Some(crate::plugins::match_rules::mime_for(extension)),
                ..Default::default()
            }),
            storage: view.clone(),
            view,
            capabilities: ResourceCapabilities {
                operations: vec![FsOperation::Read, FsOperation::List],
            },
        }
    }

    #[test]
    fn unmatched_provider_is_rejected_before_scope_or_ipc() {
        let plugin = crate::plugins::host::PluginInfo {
            schema_version: 2,
            id: "org.fiex.unmatched".into(),
            name: "Unmatched".into(),
            version: "1".into(),
            description: String::new(),
            icon: None,
            author: None,
            extensions: json!({"info":{"match":{"extensions":["zip"]}}}),
            ui_timeout_ms: 15000,
            backend_available: true,
            enabled: true,
            approved: true,
            declared_permissions: vec![],
            granted_permissions: vec![],
            requested_network_all: false,
            requested_network_domains: vec![],
            allowed_network_all: false,
            allowed_network_domains: vec![],
            requested_commands: vec![],
            allowed_commands: vec![],
            missing_commands: vec![],
        };
        let absent = std::env::temp_dir().join("fiex-match-must-not-open.txt");
        let resource = ResourceSnapshot::new(absent.to_str().unwrap(), &absent);
        let result =
            super::super::invoke_resources(&plugin, "info.resolve", json!({}), &[resource], None);
        assert_eq!(result.unwrap_err(), "provider does not match event targets");
    }

    #[test]
    fn every_resource_event_rejects_nonmatching_targets() {
        let txt = target("file", "txt", None);
        let zip = target("file", "zip", None);
        for (feature, method) in [
            ("info", "info.resolve"),
            ("preview", "preview.open"),
            ("decorations", "decorations.resolve"),
            ("contextMenus", "contextMenu.resolve"),
            ("contextMenus", "contextMenu.execute"),
            ("hooks", "hooks.invoke"),
            ("search", "search.query"),
        ] {
            let extensions =
                json!({feature: {"match": {"extensions": ["zip"], "kinds": ["file"]}}});
            assert!(
                !matches(method, &extensions, std::slice::from_ref(&txt), ""),
                "{method}"
            );
            assert!(!matches(method, &extensions, &[], ""), "{method}");
            assert!(
                matches(method, &extensions, std::slice::from_ref(&zip), ""),
                "{method}"
            );
            let extensions = json!({feature: {"match": {"extensions": "zip"}}});
            assert!(!matches(method, &extensions, &[], ""), "{method}");
        }
    }

    #[test]
    fn core_assigns_view_and_storage_before_plugin_candidate_matching() {
        let path = crate::core::volumes::applications::URI;
        let resource = ResourceSnapshot::from_metadata(
            path,
            ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        let selected = match_resources(&[resource]);
        assert_eq!(
            selected[0].view.as_ref().unwrap().provider_id,
            "core:applications"
        );
        assert_eq!(
            selected[0].storage.as_ref().unwrap().resource_type,
            ResourceType::Vfs
        );
        let extensions = json!({"contextMenus":{"match":{
            "view":{"providerIds":["core:applications"]}, "kinds":["directory"]
        }}});
        assert!(matches("contextMenu.resolve", &extensions, &selected, path));
        let local = ResourceSnapshot::from_metadata(
            "/tmp/fiex-matcher-local",
            ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        assert!(!matches(
            "contextMenu.resolve",
            &extensions,
            &match_resources(&[local]),
            "/tmp"
        ));
    }

    #[test]
    fn blank_context_menus_require_an_explicit_zero_selection() {
        let directory = crate::plugins::match_rules::directory_match_resource("/tmp");
        assert!(!matches(
            "contextMenu.resolve",
            &json!({"contextMenus": true}),
            &[],
            "/tmp"
        ));
        assert!(!matches(
            "contextMenu.resolve",
            &json!({"contextMenus": {"match": {"selection": "any"}}}),
            &[],
            "/tmp"
        ));
        assert!(matches(
            "contextMenu.resolve",
            &json!({"contextMenus": {"match": {"selection": "zero"}}}),
            &[],
            "/tmp"
        ));
        assert!(directory.is_some());
    }
}
