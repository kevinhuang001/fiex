//! Windows removable volume discovery and native mount calls.
use super::super::{super::Location, fallback};

#[cfg(all(windows, feature = "usbfs"))]
pub(crate) fn unmounted_removable() -> Vec<Location> {
    crate::core::volumes::windows_native::enumerate_unmounted_removable()
        .into_iter()
        .map(|volume| {
            let fallback = fallback::active_fallback(&volume.device);
            Location {
                name: volume.name,
                path: fallback
                    .as_ref()
                    .map(|mount| mount.path.clone())
                    .unwrap_or_default(),
                kind: "disk".into(),
                available: volume.available,
                total: volume.total,
                removable: true,
                filesystem: volume.filesystem,
                device: volume.device,
                mounted: false,
                usbfs: fallback.is_some(),
            }
        })
        .collect()
}

#[cfg(windows)]
pub fn mount_device(device: &str) -> Result<String, String> {
    fallback::clear_usb_fallback(device);
    let (path, filesystem) =
        crate::core::volumes::windows_native::mount_volume(device).map_err(|error| {
            if error.contains("No drive letter") {
                crate::i18n::t("系统未为该设备分配挂载路径").to_string()
            } else {
                error
            }
        })?;
    match std::fs::read_dir(&path) {
        Ok(_) => Ok(path),
        Err(error) => {
            if fallback::windows_native_mount_unsupported(&error) {
                let letter = path
                    .as_bytes()
                    .first()
                    .copied()
                    .filter(|value| value.is_ascii_alphabetic())
                    .map(char::from)
                    .ok_or_else(|| crate::i18n::t("系统未为该设备分配挂载路径").to_string())?;
                let raw_device = format!(r"\\.\{letter}:");
                #[cfg(feature = "usbfs")]
                if let Some(fallback) =
                    fallback::fallback_mount(device, &raw_device, &path, &filesystem)
                {
                    return fallback;
                }
            }
            Err(error.to_string())
        }
    }
}
