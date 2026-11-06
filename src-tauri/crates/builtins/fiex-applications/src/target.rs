//! Validation for launchable Applications targets.

/// Whether a target is a supported executable format, excluding URLs/documents.
pub fn is_executable_target(value: &str) -> bool {
    if value.contains("://") {
        return false;
    }
    let extension = std::path::Path::new(value)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "exe" | "com" | "bat" | "cmd" | "msc" | "cpl" | "scr" | "ps1" | "appref-ms" | "jar"
    )
}
