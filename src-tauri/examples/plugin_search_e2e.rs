//! Cross-repository test for the Core-owned plugin search aggregator.
use fiex_lib::{plugin_host, plugin_search::PluginSearchInput};

fn main() {
    fiex_lib::plugin_sandbox::run_wrapper_if_requested();
    let id = "org.fiex.mock";
    let entry = plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == id)
        .expect("mock plugin must be installed in FIEX_PLUGIN_DIR");
    assert!(entry.backend_available);
    if !entry.approved {
        plugin_host::approve_install(id).expect("approval failed");
    }
    plugin_host::set_enabled(id, true).expect("enable failed");
    let entries = fiex_lib::plugin_search::query(&PluginSearchInput {
        query: "mock".into(),
        root: "everywhere://".into(),
        hidden: false,
        ignores: Vec::new(),
        limit: Some(50),
    });
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].name, "fiex-mock-search.txt");
    plugin_host::global().stop(id);
    println!("PASS: plugin search aggregation");
}
