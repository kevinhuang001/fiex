//! Test-only executable used by Core CI and official plugin integration CI.
//! It is not included in Fiex production application bundles.
//!
//! Resolves every panel a plugin declares through the real host, including
//! panels nested inside context submenus. A manifest that advertises a panel the
//! host cannot resolve used to fail only when the user clicked it; this turns
//! that into a test failure.
use fiex_lib::plugin_host;
use serde_json::Value;

fn collect_panels(items: &[Value], depth: usize, out: &mut Vec<String>) {
    if depth > 4 {
        return;
    }
    for item in items {
        if let Some(id) = item.get("id").and_then(Value::as_str) {
            if item.get("panel").is_some() {
                out.push(id.to_string());
            }
        }
        if let Some(children) = item.get("children").and_then(Value::as_array) {
            collect_panels(children, depth + 1, out);
        }
    }
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
        let mut panels = Vec::new();
        for key in ["toolbar", "contextMenus"] {
            if let Some(items) = manifest.extensions.get(key).and_then(Value::as_array) {
                collect_panels(items, 1, &mut panels);
            }
        }
        assert!(!panels.is_empty(), "{id} declares no panels");
        for panel in &panels {
            let html = plugin_host::panel_html(&id, panel)
                .unwrap_or_else(|error| panic!("{id}/{panel}: {error}"));
            assert!(
                html.to_ascii_lowercase().contains("<html"),
                "{id}/{panel}: resolved an empty panel document"
            );
        }
        println!("PASS: {id}: {} panels resolved", panels.len());
    }
}
