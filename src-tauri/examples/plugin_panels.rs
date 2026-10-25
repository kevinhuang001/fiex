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
    for id in ids {
        let (manifest, _) = plugin_host::installed_manifests()
            .into_iter()
            .find(|(manifest, _)| manifest.id == id)
            .unwrap_or_else(|| panic!("expected plugin fixture in FIEX_PLUGIN_DIR: {id}"));
        let consent =
            plugin_host::activation_requirements(&manifest).expect("invalid permission manifest");
        plugin_host::activate(&id, &consent).expect("permission activation failed");

        let mut count = 0usize;
        for key in ["toolbar", "preview"] {
            if let Some(items) = manifest.extensions.get(key).and_then(Value::as_array) {
                for item in items {
                    let Some(panel_id) = item.get("id").and_then(Value::as_str) else {
                        continue;
                    };
                    if item.get("panel").is_some() {
                        let html = plugin_host::panel_html(&id, panel_id)
                            .unwrap_or_else(|error| panic!("{id}/{panel_id}: {error}"));
                        check_panel(&id, panel_id, &html);
                        count += 1;
                    }
                }
            }
        }

        if manifest.extensions["contextMenus"].as_bool() == Some(true) {
            let response = plugin_host::global()
                .invoke(
                    &id,
                    "contextMenu.resolve",
                    json!({
                        "selectionCount": 1,
                        "selectionNames": ["example.txt"],
                        "isDirectory": false,
                        "readOnly": false,
                        "isRemote": false
                    }),
                )
                .expect("dynamic menu resolve failed");
            let items = response["items"].as_array().expect("menu has items");
            assert!(!items.is_empty(), "dynamic menu is unexpectedly empty");
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
                let result = plugin_host::global()
                    .invoke(
                        &id,
                        "contextMenu.execute",
                        json!({
                            "itemId": item_id,
                            "paths": ["example.txt"]
                        }),
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
