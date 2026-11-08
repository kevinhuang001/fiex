//! Application icon cache shared by the module's decorations and directory
//! presentation paths.

#[cfg(feature = "desktop")]
pub fn cached_icon_with(
    source: &str,
    image_icon: impl FnOnce(&str, &std::path::Path) -> Result<(), String>,
    shell_icon: impl FnOnce(&str, &std::path::Path) -> Result<(), String>,
    generic_icon: impl FnOnce(&str, &std::path::Path) -> Result<(), String>,
) -> Option<String> {
    use sha2::{Digest, Sha256};
    use std::path::Path;

    let path = Path::new(source);
    if !source.starts_with("shell:AppsFolder\\") && !path.exists() {
        return None;
    }
    let cache = dirs::home_dir()?.join(".fiex").join("app-icons");
    std::fs::create_dir_all(&cache).ok()?;
    let stamp = std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|time| time.as_secs())
        .unwrap_or(0);
    let digest = Sha256::digest(format!("v3:{source}:{stamp}").as_bytes());
    let target = cache.join(format!("{}.png", hex::encode(&digest[..12])));
    if target.exists() {
        return Some(target.to_string_lossy().into_owned());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "ico")
        && image_icon(source, &target).is_ok()
    {
        return Some(target.to_string_lossy().into_owned());
    }
    if shell_icon(source, &target).is_ok() {
        return Some(target.to_string_lossy().into_owned());
    }
    generic_icon(source, &target).ok()?;
    Some(target.to_string_lossy().into_owned())
}

#[cfg(not(feature = "desktop"))]
pub fn cached_icon_with(
    _source: &str,
    _image_icon: impl FnOnce(&str, &std::path::Path) -> Result<(), String>,
    _shell_icon: impl FnOnce(&str, &std::path::Path) -> Result<(), String>,
    _generic_icon: impl FnOnce(&str, &std::path::Path) -> Result<(), String>,
) -> Option<String> {
    None
}
