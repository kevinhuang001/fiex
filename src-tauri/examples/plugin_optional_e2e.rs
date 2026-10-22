//! Cross-repository smoke test for the optional Archive and Checksum plugins.
use fiex_lib::plugin_host;
use serde_json::json;

fn approve(id: &str) {
    let entry = plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == id)
        .expect("plugin must be installed in FIEX_PLUGIN_DIR");
    assert!(entry.backend_available, "{id} has no native backend");
    if !entry.approved {
        plugin_host::approve_install(id).expect("approval failed");
    }
    plugin_host::set_enabled(id, true).expect("enable failed");
}

fn main() {
    let root = std::env::temp_dir().join(format!("fiex-optional-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("hello.txt");
    std::fs::write(&file, b"abc").unwrap();

    approve("org.fiex.checksum");
    let hashes = plugin_host::global()
        .invoke(
            "org.fiex.checksum",
            "checksum.calculate",
            json!({"path": file}),
        )
        .expect("checksum failed");
    let rows = hashes.as_array().expect("checksum rows");
    assert_eq!(rows[0]["algorithm"], "SHA-256");
    assert_eq!(
        rows[0]["value"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(rows[3]["value"], "900150983cd24fb0d6963f7d28e17f72");

    approve("org.fiex.archive");
    for format in ["zip", "tar.gz", "7z"] {
        let archive = root.join(format!("bundle.{format}"));
        let created = plugin_host::global()
            .invoke(
                "org.fiex.archive",
                "archive.compress",
                json!({
                    "paths": [file],
                    "destination": archive,
                    "format": format,
                    "ignores": []
                }),
            )
            .expect("compress failed");
        assert_eq!(created.as_str(), Some(archive.to_string_lossy().as_ref()));
        assert!(archive.is_file(), "{format} archive was not created");
        let output = root.join(format!("out-{format}"));
        std::fs::create_dir_all(&output).unwrap();
        let extracted = plugin_host::global()
            .invoke(
                "org.fiex.archive",
                "archive.extract",
                json!({"path": archive, "destination": output}),
            )
            .expect("extract failed");
        let extracted = extracted.as_str().expect("extract path").to_string();
        assert_eq!(
            std::fs::read(std::path::Path::new(&extracted).join("hello.txt")).unwrap(),
            b"abc",
            "{format} roundtrip"
        );
    }

    plugin_host::global().stop("org.fiex.archive");
    plugin_host::global().stop("org.fiex.checksum");
    std::fs::remove_dir_all(&root).ok();
    println!("PASS: Archive and Checksum plugins through the sandboxed host");
}
