//! Host integration: static toolbar/preview panels and dynamic menu panels.
use fiex_lib::plugin_host;
use serde_json::{json, Value};

fn check_panel(id: &str, panel_id: &str, html: &str) {
    assert!(
        html.to_ascii_lowercase().contains("<html"),
        "{id}/{panel_id}: empty panel document"
    );
}

fn main() {
    fiex_lib::plugin_sandbox::run_wrapper_if_requested();
    let ids: Vec<String> = std::env::args().skip(1).collect();
    assert!(
        !ids.is_empty(),
        "usage: cargo run --example plugin_panels -- <plugin-id> [plugin-id...]"
    );
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("example.txt");
    std::fs::write(&file, "panel fixture").unwrap();
    let resources = [fiex_lib::plugin_resource_context::ResourceSnapshot::new(
        file.to_str().unwrap(),
        &file,
    )];
    for id in ids {
        let (manifest, _) = plugin_host::installed_manifests()
            .into_iter()
            .find(|(manifest, _)| manifest.id == id)
            .unwrap_or_else(|| panic!("expected plugin fixture in FIEX_PLUGIN_DIR: {id}"));
        let consent =
            plugin_host::activation_requirements(&manifest).expect("invalid permission manifest");
        if !plugin_host::catalog()
            .into_iter()
            .find(|plugin| plugin.id == id)
            .is_some_and(|plugin| plugin.approved)
        {
            plugin_host::approve_install(&id).expect("install approval failed");
        }
        plugin_host::activate(&id, &consent).expect("permission activation failed");

        let mut count = 0usize;
        for key in ["toolbar", "preview"] {
            if let Some(item) = manifest.extensions.get(key).and_then(Value::as_object) {
                if item.get("panel").is_some() {
                    let html = plugin_host::panel_html(&id, &id)
                        .unwrap_or_else(|error| panic!("{id}: {error}"));
                    check_panel(&id, &id, &html);
                    count += 1;
                }
            }
        }

        if manifest.extensions["contextMenus"].as_bool() == Some(true) {
            let response = fiex_lib::plugin_events::call_owner(
                "contextMenu.resolve",
                &id,
                json!({}),
                &resources,
                directory.path().to_str(),
            )
            .expect("dynamic menu resolve failed");
            let mut items = response["items"]
                .as_array()
                .expect("menu has items")
                .clone();
            assert!(
                !items.is_empty(),
                "dynamic menu is unexpectedly empty: {id}"
            );
            for item in items.clone() {
                if item["kind"] == "submenu" {
                    let children = fiex_lib::plugin_events::call_owner(
                        "contextMenu.resolve",
                        &id,
                        json!({"parentId":item["id"]}),
                        &resources,
                        directory.path().to_str(),
                    )
                    .expect("lazy submenu resolve failed");
                    items.extend(
                        children["items"]
                            .as_array()
                            .expect("submenu has items")
                            .clone(),
                    );
                }
            }
            let mut found_panel = false;
            for item in items {
                if item["kind"].as_str() != Some("action") {
                    continue;
                }
                let Some(item_id) = item["id"].as_str() else {
                    continue;
                };
                if id == "org.fiex.archive" && item_id != "archive.advanced" {
                    continue;
                }
                if id == "org.fiex.gpg" && item_id != "gpg.encrypt" {
                    continue;
                }
                let result = fiex_lib::plugin_events::call_owner(
                    "contextMenu.execute",
                    &id,
                    json!({"itemId":item_id}),
                    &resources,
                    directory.path().to_str(),
                )
                .expect("menu panel execute failed");
                if result["kind"] == "panel" {
                    let entry = result["entry"].as_str().expect("missing panel entry");
                    let path = plugin_host::plugin_view_asset(&id, entry)
                        .expect("dynamic panel asset failed");
                    let html = std::fs::read_to_string(path).unwrap();
                    check_panel(&id, item_id, &html);
                    count += 1;
                    found_panel = true;
                }
            }
            if ["org.fiex.archive", "org.fiex.gpg"].contains(&id.as_str()) {
                assert!(found_panel, "{id} did not return a dynamic panel");
            }
        }
        assert!(count > 0, "{id} has no resolvable panels");
        println!("PASS: {id}: {count} panels resolved");
    }
}
