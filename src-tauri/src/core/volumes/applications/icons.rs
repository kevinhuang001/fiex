#[cfg(feature = "desktop")]
use std::path::Path;

/// Render a real application bundle or shortcut icon into a PNG the asset
/// protocol can show. Core decorations use this only for system-owned apps.
#[cfg(feature = "desktop")]
pub fn cached_icon(source: &str) -> Option<String> {
    let path = Path::new(source);
    if !source.starts_with("shell:AppsFolder\\") && !path.exists() {
        return None;
    }
    let cache = dirs::home_dir()?.join(".fiex").join("app-icons");
    std::fs::create_dir_all(&cache).ok()?;
    use sha2::{Digest, Sha256};
    let stamp = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|t| t.as_secs())
        .unwrap_or(0);
    let digest = Sha256::digest(format!("v3:{source}:{stamp}").as_bytes());
    let target = cache.join(format!("{}.png", hex::encode(&digest[..12])));
    if target.exists() {
        return Some(target.to_string_lossy().into_owned());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "ico") {
        if let Ok(image) = image::open(path) {
            image.thumbnail(128, 128).save(&target).ok()?;
            return Some(target.to_string_lossy().into_owned());
        }
    }
    #[cfg(target_os = "windows")]
    if super::windows_shell::extract_icon(source, &target).is_ok() {
        return Some(target.to_string_lossy().into_owned());
    }
    let icon = file_icon_provider::get_file_icon(source, 128).ok()?;
    let image = image::RgbaImage::from_raw(icon.width, icon.height, icon.pixels)?;
    image.save(&target).ok()?;
    Some(target.to_string_lossy().into_owned())
}

#[cfg(not(feature = "desktop"))]
pub fn cached_icon(_source: &str) -> Option<String> {
    None
}
