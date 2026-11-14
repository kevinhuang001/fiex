//! Runs once to persist a VFS view, then in a fresh Core process to reopen it
//! through the registered plugin provider and real JSON-RPC host transport.
use fiex_lib::plugins::{host as plugin_host, mounts, resource::fs_access::Scope, vfs};
use serde_json::json;
use std::path::PathBuf;

const PLUGIN_ID: &str = "org.fiex.mock-vfs";

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
}

fn verify_vfs(uri: &str) {
    let listing = vfs::list(uri, false, &[]).expect("registered VFS provider list failed");
    assert_eq!(listing.entries.len(), 1);
    let entry = &listing.entries[0];
    assert_eq!(entry.name, "fixture.txt");
    assert_eq!(entry.size, 7);
    let stat = vfs::stat(&entry.path).expect("registered VFS provider stat failed");
    assert_eq!(stat.name, "fixture.txt");
    assert_eq!(stat.size, 7);
}

fn main() {
    fiex_lib::plugins::security::sandbox::run_wrapper_if_requested();
    let phase = std::env::args()
        .nth(1)
        .expect("usage: plugin_vfs_smoke -- <fresh|restart>");
    assert!(
        phase == "fresh" || phase == "restart",
        "unknown phase: {phase}"
    );

    let entry = plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == PLUGIN_ID)
        .expect("VFS plugin fixture is not installed");
    assert!(entry.backend_available, "native backend is missing");
    if phase == "fresh" {
        assert!(!entry.approved, "fresh run must start without consent");
        let (manifest, _) = plugin_host::installed_manifests()
            .into_iter()
            .find(|(manifest, _)| manifest.id == PLUGIN_ID)
            .unwrap();
        let consent = plugin_host::activation_requirements(&manifest).unwrap();
        plugin_host::approve_install(PLUGIN_ID).expect("install approval failed");
        plugin_host::activate(PLUGIN_ID, &consent).expect("permission activation failed");
    } else {
        assert!(
            entry.approved && entry.enabled,
            "restart must reuse consent"
        );
    }

    plugin_host::global()
        .invoke(PLUGIN_ID, "plugin.ping", json!({}))
        .expect("VFS plugin handshake failed");

    let data = data_home();
    let root = data.join("fiex-mock-vfs-root");
    let uri_file = data.join("fiex-mock-vfs-uri.txt");
    std::fs::create_dir_all(&root).expect("could not create persisted VFS root");
    let uri = if phase == "fresh" {
        let scope = Scope::selected(&root, std::slice::from_ref(&root))
            .expect("could not authorize fixture VFS root");
        let resource_root = root.to_string_lossy().into_owned();
        let result = mounts::open_view(
            PLUGIN_ID,
            &json!({
                "contributionId":"fixture",
                "resourceRoot":resource_root,
                "viewContext":{}
            }),
            &scope,
        )
        .expect("Core could not open the VFS view");
        let uri = result
            .as_str()
            .expect("Core returned no VFS URI")
            .to_string();
        mounts::retain("plugin-vfs-smoke", &[], std::slice::from_ref(&uri))
            .expect("Core could not persist the VFS view");
        std::fs::write(&uri_file, &uri).expect("could not persist VFS URI for restart");
        uri
    } else {
        std::fs::read_to_string(uri_file).expect("fresh Core process did not persist the VFS URI")
    };

    // Deliberately crash the SDK provider during a real VFS operation. Core
    // must surface that failure, replace the process, and serve the retry.
    let failure = match vfs::list(&uri, true, &[]) {
        Ok(_) => panic!("the fixture must terminate during the hidden listing"),
        Err(error) => error,
    };
    assert!(failure.contains("control channel"), "{failure}");

    // This reopens the provider through ResourceBroker's VFS routing and the
    // plugin host's actual framed JSON-RPC process boundary after recovery.
    verify_vfs(&uri);
    plugin_host::global().stop(PLUGIN_ID);
    println!("PASS: {phase}: reopened registered VFS Provider through Core/Host IPC");
}
