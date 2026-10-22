//! Test-only executable used by Core CI and official plugin integration CI.
//! It is not included in Fiex production application bundles.
use fiex_lib::plugin_host;
use serde_json::json;

fn main() {
    let id = std::env::args()
        .nth(1)
        .expect("usage: cargo run --example plugin_smoke -- <plugin-id>");
    let entry = plugin_host::catalog()
        .into_iter()
        .find(|p| p.id == id)
        .expect("expected plugin fixture in FIEX_PLUGIN_DIR");
    assert!(entry.backend_available, "native backend is missing");
    assert!(!entry.approved, "test must start with no prior permission grant");
    assert!(
        plugin_host::global()
            .invoke(&id, "plugin.ping", json!({}))
            .is_err(),
        "an unapproved plugin must never start"
    );
    // Simulates explicit user consent in an isolated test profile only.
    plugin_host::approve_install(&id).expect("permission approval failed");
    let pong = plugin_host::global()
        .invoke(&id, "plugin.ping", json!({}))
        .expect("Core-to-plugin process IPC failed");
    assert_eq!(pong["alive"], true);
    if id == "org.fiex.mock" {
        let echoed = plugin_host::global()
            .invoke(&id, "mock.echo", json!({"message":"中文 IPC","value":42}))
            .expect("mock plugin echo failed");
        assert_eq!(echoed["echo"]["message"], "中文 IPC");
        assert_eq!(echoed["echo"]["value"], 42);
    }
    plugin_host::set_enabled(&id, false).expect("disable failed");
    assert!(
        plugin_host::global()
            .invoke(&id, "plugin.ping", json!({}))
            .is_err(),
        "disabled plugin must not answer requests"
    );
    plugin_host::set_enabled(&id, true).expect("re-enable failed");
    let pong = plugin_host::global()
        .invoke(&id, "plugin.ping", json!({}))
        .expect("plugin restart failed");
    assert_eq!(pong["alive"], true);
    plugin_host::global().stop(&id);
    println!("PASS: {id}: approval, process handshake, IPC, revoke, restart");
}
