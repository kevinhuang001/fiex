//! Package validation: ZIP extraction, manifest loading, size limits and the
//! on-disk plugin installation root.
use crate::plugins::host as plugin_host;
use std::{
    fs,
    io::{Cursor, Write},
    path::{Component, Path, PathBuf},
};
use zip::ZipArchive;

pub(super) const MAX_DOWNLOAD: u64 = 60 * 1024 * 1024;
const MAX_UNPACKED: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 2_000;

pub fn install_root() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|p| p.join("fiex").join("plugins"))
        .ok_or("application data directory unavailable".into())
}

pub(super) fn normal_relative(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}

pub(super) fn unpack(package: &[u8], output: &Path) -> Result<(), String> {
    let reader = Cursor::new(package);
    let mut zip = ZipArchive::new(reader).map_err(|e| format!("invalid plugin ZIP: {e}"))?;
    if zip.len() > MAX_FILES {
        return Err("plugin contains too many files".into());
    }
    let mut unpacked = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let rel = entry
            .enclosed_name()
            .ok_or("unsafe ZIP member path")?
            .to_path_buf();
        if !normal_relative(&rel) {
            return Err("ZIP member escapes plugin directory".into());
        }
        // Disallow symbolic links, devices and other special file types.
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                return Err("plugin ZIP contains a symlink or special file".into());
            }
        }
        unpacked = unpacked
            .checked_add(entry.size())
            .ok_or("plugin size overflow")?;
        if unpacked > MAX_UNPACKED {
            return Err("plugin unpacked data too large".into());
        }
        let destination = output.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut file = fs::File::create(&destination).map_err(|e| e.to_string())?;
            let count = std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
            if count != entry.size() {
                return Err("incomplete plugin ZIP member".into());
            }
            file.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Read and validate the manifest at the root of an unpacked package.
pub(super) fn read_manifest(root: &Path) -> Result<(Vec<u8>, plugin_host::PluginManifest), String> {
    let manifest_bytes = fs::read(root.join("manifest.json"))
        .map_err(|_| "manifest.json must be at the ZIP root")?;
    if manifest_bytes.len() > 1024 * 1024 {
        return Err("manifest too large".into());
    }
    let manifest: plugin_host::PluginManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("manifest invalid: {e}"))?;
    if manifest.schema_version != 2 {
        return Err(format!(
            "unsupported plugin manifest schema {}; this Core requires schema 2",
            manifest.schema_version
        ));
    }
    if !crate::plugins::themes::is_theme(&manifest) {
        let declared = manifest.runtime.protocol_version.trim();
        // Accept "6" as the same series as "6.0"; reject older major series
        // explicitly rather than reporting a generic parse failure.
        let same_series = declared.trim_end_matches(".0")
            == crate::plugin_sdk::PROTOCOL_VERSION.trim_end_matches(".0");
        if declared.is_empty() || !same_series {
            let declared = if declared.is_empty() {
                "missing"
            } else {
                declared
            };
            return Err(format!(
                "plugin protocol {declared} is incompatible with Core {}; install a plugin built for protocol {}",
                crate::plugin_sdk::PROTOCOL_VERSION,
                crate::plugin_sdk::PROTOCOL_VERSION
            ));
        }
    }
    if !plugin_host::valid_id(&manifest.id) {
        return Err("invalid plugin id".into());
    }
    Ok((manifest_bytes, manifest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn zip_slip_is_not_accepted() {
        assert!(!normal_relative(Path::new("../outside")));
        assert!(!normal_relative(Path::new("/etc/passwd")));
    }

    fn manifest(protocol: &str) -> serde_json::Value {
        json!({
            "schemaVersion": 2,
            "id": "org.example.test",
            "name": "Test",
            "version": "1.0.0",
            "runtime": {"protocolVersion": protocol}
        })
    }

    #[test]
    fn import_reports_the_declared_protocol_instead_of_a_generic_error() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            serde_json::to_vec(&manifest("5.0")).unwrap(),
        )
        .unwrap();
        let error = read_manifest(dir.path()).unwrap_err();
        assert!(error.contains("5.0"), "{error}");
        assert!(
            error.contains(crate::plugin_sdk::PROTOCOL_VERSION),
            "{error}"
        );
    }

    #[test]
    fn import_accepts_the_same_protocol_series() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            serde_json::to_vec(&manifest("6")).unwrap(),
        )
        .unwrap();
        assert!(read_manifest(dir.path()).is_ok());
    }
}
