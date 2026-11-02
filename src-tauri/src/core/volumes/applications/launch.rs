use super::{platform, store::is_user, store::stored_path, URI};
#[cfg(any(target_os = "windows", test))]
use std::path::Path;

pub fn open(encoded: &str) -> Result<(), String> {
    let uri = format!("{URI}{encoded}");
    if is_user(&uri) {
        return platform::open_stored(&stored_path(&uri)?);
    }
    platform::platform_open(encoded)
}

/// Executable file types that the Applications view is allowed to launch.
/// Documents and website shortcuts share the Start Menu namespace, so the
/// enumeration has to reject them explicitly.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[cfg(any(target_os = "windows", test))]
pub(crate) fn is_executable_target(value: &str) -> bool {
    if value.contains("://") {
        return false;
    }
    let extension = Path::new(value)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "exe" | "com" | "bat" | "cmd" | "msc" | "cpl" | "scr" | "ps1" | "appref-ms" | "jar"
    )
}
