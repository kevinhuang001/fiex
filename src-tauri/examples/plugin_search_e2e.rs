//! Cross-repository test for one explicitly selected SearchProvider.
use fiex_lib::{
    core::search::SearchMode, plugins::host as plugin_host,
    plugins::resolvers::search::PluginSearchInput,
};

fn main() {
    fiex_lib::plugins::security::sandbox::run_wrapper_if_requested();
    let id = "org.fiex.mock";
    let entry = plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == id)
        .expect("mock plugin must be installed in FIEX_PLUGIN_DIR");
    assert!(entry.backend_available);
    let (manifest, _) = plugin_host::installed_manifests()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .unwrap();
    if !plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == id)
        .is_some_and(|plugin| plugin.approved)
    {
        plugin_host::approve_install(id).expect("install approval failed");
    }
    plugin_host::activate(
        id,
        &plugin_host::activation_requirements(&manifest).unwrap(),
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fiex-mock-search.txt"), "mock result").unwrap();
    let request = PluginSearchInput {
        query: "mock".into(),
        root: root.path().to_string_lossy().into_owned(),
        hidden: false,
        mode: SearchMode::Name,
        ignores: Vec::new(),
        limit: Some(50),
    };
    let engine = fiex_lib::plugins::resolvers::search::engines(&request)
        .into_iter()
        .find(|engine| engine.resolver.id == "plugin:org.fiex.mock")
        .expect("the activated mock must advertise filename search");
    assert!(engine.supports(SearchMode::Name));
    let (entries, truncated) = fiex_lib::plugins::resolvers::search::run(
        &engine,
        &request,
        || false,
    )
    .expect("single-provider search query");
    assert!(!truncated);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].name, "fiex-mock-search.txt");
    plugin_host::global().stop(id);
    println!("PASS: direct typed SearchProvider integration");
}
