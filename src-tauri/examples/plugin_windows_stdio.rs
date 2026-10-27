//! Regression for Explorer-launched Fiex: IPC must work without host stdio.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use fiex_lib::{plugins::host as plugin_host, plugins::security::sandbox as plugin_sandbox};
    use serde_json::json;
    use std::{os::windows::process::CommandExt, process::Command};

    // The re-executed sandbox launcher must run before test-mode handling.
    plugin_sandbox::run_wrapper_if_requested();
    let mode = std::env::args().nth(1);
    if mode.is_none() {
        let diagnostics = tempfile::tempdir().expect("worker diagnostic directory");
        for mode in ["missing-stderr", "missing-all"] {
            let report = diagnostics.path().join(format!("{mode}.txt"));
            let failure = || {
                std::fs::read_to_string(&report)
                    .unwrap_or_else(|_| "worker produced no panic diagnostic".into())
            };
            let mut child = Command::new(std::env::current_exe().unwrap())
                .arg(mode)
                .arg(&report)
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .spawn()
                .expect("start console-free regression worker");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "{mode} failed: {status}: {}", failure());
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "{mode}: plugin IPC stalled without a console: {}",
                        failure()
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
        return;
    }

    // The worker deliberately removes stderr below. Preserve failures through
    // a separate file so the parent can include the actual error in CI logs.
    let report = std::env::args().nth(2).expect("worker diagnostic path");
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::write(&report, info.to_string());
    }));

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetStdHandle(id: u32, handle: *mut std::ffi::c_void) -> i32;
    }
    let handles: &[u32] = match mode.as_deref() {
        Some("missing-stderr") => &[(-12i32) as u32],
        Some("missing-all") => &[(-10i32) as u32, (-11i32) as u32, (-12i32) as u32],
        _ => panic!("unexpected regression worker mode"),
    };
    for &id in handles {
        // SAFETY: this single-threaded worker deliberately removes its standard
        // handles before creating any plugin process or background thread.
        assert_ne!(unsafe { SetStdHandle(id, std::ptr::null_mut()) }, 0);
    }
    let id = "org.fiex.mock";
    let (manifest, _) = plugin_host::installed_manifests()
        .into_iter()
        .find(|(manifest, _)| manifest.id == id)
        .expect("mock plugin fixture");
    // Earlier smoke tests and the first worker may already have authorized the
    // fixture. Installation approval intentionally rejects repeated approval.
    let entry = plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == id)
        .expect("mock plugin catalog entry");
    if !entry.approved {
        plugin_host::approve_install(id).unwrap();
    }
    plugin_host::activate(
        id,
        &plugin_host::activation_requirements(&manifest).unwrap(),
    )
    .unwrap();
    let result = (|| {
        let pong = plugin_host::global().invoke(id, "plugin.ping", json!({}))?;
        assert_eq!(pong["alive"], true);
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("stdio.txt");
        std::fs::write(&file, "console-free IPC").unwrap();
        // Exercises the reverse SDK host.fs request as well as the handshake.
        let read = plugin_host::global().invoke_selected(
            id,
            "mock.readFile",
            json!({"path":"stdio.txt"}),
            root.path().to_str().unwrap(),
            &[file],
        )?;
        assert_eq!(read, "console-free IPC");
        Ok::<_, String>(())
    })();
    plugin_host::global().stop(id);
    result.expect("console-free bidirectional plugin IPC");
}

#[cfg(not(windows))]
fn main() {}
