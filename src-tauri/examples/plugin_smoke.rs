//! Test-only executable used by Core CI and official plugin integration CI.
//! It is not included in Fiex production application bundles.
use fiex_lib::plugin_host;
use serde_json::json;

fn main() {
    fiex_lib::plugin_sandbox::run_wrapper_if_requested();
    let id = std::env::args()
        .nth(1)
        .expect("usage: cargo run --example plugin_smoke -- <plugin-id>");
    let entry = plugin_host::catalog()
        .into_iter()
        .find(|p| p.id == id)
        .expect("expected plugin fixture in FIEX_PLUGIN_DIR");
    assert!(entry.backend_available, "native backend is missing");
    assert!(
        !entry.approved,
        "test must start with no prior permission grant"
    );
    assert!(
        plugin_host::global()
            .invoke(&id, "plugin.ping", json!({}))
            .is_err(),
        "an unapproved plugin must never start"
    );
    // Simulates explicit user consent in an isolated test profile only.
    let (manifest, _) = plugin_host::installed_manifests()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .unwrap();
    let consent = plugin_host::activation_requirements(&manifest).unwrap();
    plugin_host::activate(&id, &consent).expect("permission activation failed");
    let pong = plugin_host::global()
        .invoke(&id, "plugin.ping", json!({}))
        .expect("Core-to-plugin process IPC failed");
    assert_eq!(pong["alive"], true);
    let test_directory = tempfile::tempdir().unwrap();
    if id == "org.fiex.mock" {
        let echoed = plugin_host::global()
            .invoke(&id, "mock.echo", json!({"message":"中文 IPC","value":42}))
            .expect("mock plugin echo failed");
        assert_eq!(echoed["echo"]["message"], "中文 IPC");
        assert_eq!(echoed["echo"]["value"], 42);
        let search = plugin_host::global()
            .invoke(&id, "search.query", json!({"query":"mock"}))
            .expect("mock search failed");
        assert_eq!(search["entries"][0]["name"], "fiex-mock-search.txt");
        let root = &test_directory;
        let secret = root.path().join("secret.txt");
        std::fs::write(&secret, "host-only").unwrap();
        assert!(plugin_host::global()
            .invoke(&id, "mock.readFile", json!({"path":"secret.txt"}))
            .is_err());
        let read = plugin_host::global()
            .invoke_selected(
                &id,
                "mock.readFile",
                json!({"path":"secret.txt"}),
                root.path().to_str().unwrap(),
                std::slice::from_ref(&secret),
            )
            .expect("SDK file read failed");
        assert_eq!(read, "host-only");
        let task = plugin_host::global()
            .invoke_selected(
                &id,
                "mock.spawnTask",
                json!({"path":"secret.txt"}),
                root.path().to_str().unwrap(),
                std::slice::from_ref(&secret),
            )
            .expect("background task did not start");
        let task_id = task["taskId"].as_str().expect("task id").to_string();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let status = plugin_host::global()
                .task(&task_id)
                .expect("Core did not register the plugin task");
            if status.status == "running" || status.status == "cancelling" {
                assert!(
                    std::time::Instant::now() < deadline,
                    "background task did not finish"
                );
                std::thread::sleep(std::time::Duration::from_millis(25));
                continue;
            }
            assert_eq!(status.status, "completed", "{status:?}");
            assert_eq!(status.result, "host-only");
            assert_eq!(status.total, Some(1));
            break;
        }
        let stale = plugin_host::global()
            .invoke_selected(
                &id,
                "mock.captureTaskScope",
                json!({"path":"secret.txt"}),
                root.path().to_str().unwrap(),
                std::slice::from_ref(&secret),
            )
            .expect("task scope capture did not start");
        let stale_id = stale["taskId"].as_str().expect("stale task id").to_string();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let status = plugin_host::global()
                .task(&stale_id)
                .expect("stale task was not registered");
            if status.status == "running" || status.status == "cancelling" {
                assert!(
                    std::time::Instant::now() < deadline,
                    "task scope capture did not finish"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            assert_eq!(status.status, "completed", "{status:?}");
            break;
        }
        let denied = plugin_host::global()
            .invoke(&id, "mock.useFinishedTaskScope", json!({}))
            .expect_err("a finished task must not authorize a new file handle");
        assert!(
            denied.to_ascii_lowercase().contains("authorization")
                || denied.to_ascii_lowercase().contains("expired"),
            "{denied}"
        );
        let cancelled_task = plugin_host::global()
            .invoke(&id, "mock.longTask", json!({}))
            .expect("cancellable task did not start");
        let cancelled_id = cancelled_task["taskId"]
            .as_str()
            .expect("cancellable task id")
            .to_string();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while plugin_host::global()
            .task(&cancelled_id)
            .is_none_or(|task| task.status != "running")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "cancellable task never started"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        plugin_host::global()
            .cancel_task(&cancelled_id)
            .expect("Core could not cancel the plugin task");
        loop {
            let status = plugin_host::global()
                .task(&cancelled_id)
                .expect("cancellable task disappeared");
            if status.status == "cancelled" {
                break;
            }
            assert_eq!(status.status, "cancelling");
            assert!(
                std::time::Instant::now() < deadline,
                "cancellable task did not stop"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let blocked = plugin_host::global()
            .invoke(&id, "mock.directFile", json!({"path":secret}))
            .expect("direct filesystem access must be blocked");
        assert_eq!(blocked["blocked"], true);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let blocked = plugin_host::global()
            .invoke(
                &id,
                "mock.directNet",
                json!({"address":listener.local_addr().unwrap().to_string()}),
            )
            .expect("direct sockets must be blocked");
        assert_eq!(blocked["blocked"], true);
        let blocked = plugin_host::global()
            .invoke(&id, "mock.directExec", json!({}))
            .expect("direct execution must be blocked");
        assert_eq!(blocked["blocked"], true);
        let exec = plugin_host::global()
            .invoke(&id, "mock.exec", json!({}))
            .expect("host exec proxy failed");
        assert!(
            exec["stdout"]
                .as_str()
                .is_some_and(|text| text.contains("git version")),
            "{exec}"
        );
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
