//! Verifies that one resident plugin process serves overlapping requests.
//!
//! The fixture method waits on a two-party barrier. If Core serialized plugin
//! requests, the first call would block forever; with concurrent dispatch both
//! calls meet and return.
use fiex_lib::plugins::host as plugin_host;
use serde_json::json;

fn main() {
    fiex_lib::plugins::security::sandbox::run_wrapper_if_requested();
    let id = "org.fiex.mock";
    let (manifest, _) = plugin_host::installed_manifests()
        .into_iter()
        .find(|(manifest, _)| manifest.id == id)
        .expect("mock plugin must be installed in FIEX_PLUGIN_DIR");
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
    let first = std::thread::spawn(|| {
        plugin_host::global()
            .invoke("org.fiex.mock", "mock.rendezvous", json!({"request": 1}))
            .expect("first concurrent request failed")
    });
    let second = std::thread::spawn(|| {
        plugin_host::global()
            .invoke("org.fiex.mock", "mock.rendezvous", json!({"request": 2}))
            .expect("second concurrent request failed")
    });
    assert_eq!(first.join().unwrap()["met"], true);
    assert_eq!(second.join().unwrap()["met"], true);
    plugin_host::global().stop(id);
    println!("PASS: concurrent requests share one resident plugin process");
}
