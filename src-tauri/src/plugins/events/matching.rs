//! The same predicate gates discovery and scope preparation before native IPC.
use super::{event, ResourceSnapshot};
use serde_json::{json, Value};

pub(super) fn matches(
    method: &str,
    extensions: &Value,
    metadata: &[Value],
    directory: &str,
) -> bool {
    let Some(contribution) = event(method).and_then(|event| extensions.get(event.extension)) else {
        return false;
    };
    if !contribution.is_object() && contribution.as_bool() != Some(true) {
        return false;
    }
    // Connection menus describe a connection, not file targets; the plugin
    // receives the connection summary and decides which items fit it.
    if method.starts_with("connectionContextMenu.") {
        return true;
    }
    let Ok(rules) = crate::plugins::match_rules::MatchRules::from_contribution(contribution) else {
        return false;
    };
    // The blank canvas is selection 0. A plugin must opt into it explicitly;
    // "any" and the default predicate keep meaning a non-empty selection.
    if method == "contextMenu.resolve"
        && metadata.is_empty()
        && rules.selection.as_deref() != Some("zero")
    {
        return false;
    }
    rules.matches_event(metadata, directory)
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
        &metadata(resources),
        directory.unwrap_or(""),
    )
}

pub fn metadata(resources: &[ResourceSnapshot]) -> Vec<Value> {
    resources
        .iter()
        .map(|resource| {
            let mut metadata =
                serde_json::to_value(resource.metadata()).unwrap_or_else(|_| json!({}));
            if !metadata.is_object() {
                metadata = json!({});
            }
            metadata["operations"] = json!(crate::plugins::provider_registry::FsOperation::ALL
                .into_iter()
                .filter(|op| resource
                    .operations
                    .unwrap_or_else(|| crate::plugins::resource::file_capabilities::for_path(
                        &resource.display
                    ))
                    .allows(*op))
                .map(|op| op.as_str())
                .collect::<Vec<_>>());
            // Only the trusted Core mount and view registries may establish
            // owner identities. Never accept a provider-supplied owner in
            // resource metadata or allow it to change the IO route.
            let (view, storage) = crate::plugins::match_rules::locations_for(&resource.display);
            if let Some(view) = view {
                metadata["view"] = view;
            }
            if let Some(storage) = storage {
                metadata["storage"] = storage;
            }
            metadata
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // A scope would fail to canonicalize this absent file. The mismatch
        // must be returned first without touching the path or starting a host.
        let absent = std::env::temp_dir().join("fiex-match-must-not-open.txt");
        let resource = ResourceSnapshot::new(absent.to_str().unwrap(), &absent);
        let result =
            super::super::invoke_resources(&plugin, "info.resolve", json!({}), &[resource], None);
        assert_eq!(result.unwrap_err(), "provider does not match event targets");
    }

    #[test]
    fn every_resource_event_rejects_nonmatching_targets() {
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
                !matches(
                    method,
                    &extensions,
                    &[json!({"extension":"txt", "kind":"file"})],
                    ""
                ),
                "{method}"
            );
            assert!(!matches(method, &extensions, &[], ""), "{method}");
            assert!(
                matches(
                    method,
                    &extensions,
                    &[json!({"extension":"zip", "kind":"file"})],
                    ""
                ),
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
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        let selected_metadata = metadata(&[resource]);
        assert_eq!(
            selected_metadata[0]["view"]["providerId"],
            "core:applications"
        );
        assert_eq!(selected_metadata[0]["storage"]["type"], "vfs");
        let extensions = json!({"contextMenus":{"match":{
            "view":{"providerIds":["core:applications"]},
            "kinds":["directory"]
        }}});
        assert!(matches(
            "contextMenu.resolve",
            &extensions,
            &selected_metadata,
            path
        ));
        let local = ResourceSnapshot::from_metadata(
            "/tmp/fiex-matcher-local",
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        assert!(!matches(
            "contextMenu.resolve",
            &extensions,
            &metadata(&[local]),
            "/tmp"
        ));
    }

    #[test]
    fn blank_context_menus_require_an_explicit_zero_selection() {
        let directory = "/tmp";
        assert!(!matches(
            "contextMenu.resolve",
            &json!({"contextMenus": true}),
            &[],
            directory
        ));
        assert!(!matches(
            "contextMenu.resolve",
            &json!({"contextMenus": {"match": {"selection": "any"}}}),
            &[],
            directory
        ));
        assert!(matches(
            "contextMenu.resolve",
            &json!({"contextMenus": {"match": {"selection": "zero"}}}),
            &[],
            directory
        ));
    }
}
