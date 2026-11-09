//! macOS removable volume discovery and native mount calls.
use super::super::{super::Location, fallback};

#[cfg(target_os = "macos")]
pub(crate) fn visible_mount(path: &str) -> bool {
    // APFS exposes the data volume and several synthetic volumes from the same
    // container. Only the root and user-mounted volumes should be listed.
    !path.starts_with("/System/Volumes/") && (path == "/" || path.starts_with("/Volumes/"))
}

#[cfg(target_os = "macos")]
fn disk_info(id: &str) -> Option<plist::Dictionary> {
    let output = std::process::Command::new("diskutil")
        .args(["info", "-plist", id])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    plist::Value::from_reader(std::io::Cursor::new(output.stdout))
        .ok()?
        .into_dictionary()
}

#[cfg(target_os = "macos")]
fn string_field(info: &plist::Dictionary, key: &str) -> String {
    info.get(key)
        .and_then(plist::Value::as_string)
        .unwrap_or_default()
        .to_string()
}

#[cfg(target_os = "macos")]
fn int_field(info: &plist::Dictionary, key: &str) -> u64 {
    info.get(key)
        .and_then(|value| {
            value
                .as_unsigned_integer()
                .or_else(|| value.as_signed_integer().map(|value| value.max(0) as u64))
        })
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
pub(crate) fn unmounted_removable() -> Vec<Location> {
    let mut out = Vec::new();
    let Ok(output) = std::process::Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
    else {
        return out;
    };
    let Ok(value) = plist::Value::from_reader(std::io::Cursor::new(output.stdout)) else {
        return out;
    };
    let Some(root) = value.as_dictionary() else {
        return out;
    };
    let Some(disks) = root
        .get("AllDisksAndPartitions")
        .and_then(plist::Value::as_array)
    else {
        return out;
    };
    for disk in disks {
        let Some(disk) = disk.as_dictionary() else {
            continue;
        };
        let Some(disk_id) = disk
            .get("DeviceIdentifier")
            .and_then(plist::Value::as_string)
        else {
            continue;
        };
        let Some(info) = disk_info(disk_id) else {
            continue;
        };
        let removable = info
            .get("RemovableMedia")
            .and_then(plist::Value::as_boolean)
            .unwrap_or(false)
            || info
                .get("Ejectable")
                .and_then(plist::Value::as_boolean)
                .unwrap_or(false);
        if !removable {
            continue;
        }
        let partitions = disk
            .get("Partitions")
            .and_then(plist::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for partition in partitions {
            let Some(partition) = partition.as_dictionary() else {
                continue;
            };
            let Some(id) = partition
                .get("DeviceIdentifier")
                .and_then(plist::Value::as_string)
            else {
                continue;
            };
            let Some(info) = disk_info(id) else {
                continue;
            };
            let mount = string_field(&info, "MountPoint");
            if !mount.is_empty() && mount != "Not mounted" && mount != "None" {
                continue;
            }
            let filesystem = {
                let name = string_field(&info, "FilesystemName");
                if name.is_empty() {
                    string_field(&info, "FilesystemType")
                } else {
                    name
                }
            };
            let name = {
                let name = string_field(&info, "VolumeName");
                if name.is_empty() {
                    string_field(&info, "MediaName")
                } else {
                    name
                }
            };
            let device = format!("/dev/{id}");
            let virtual_path = fallback::virtual_path(&device, &filesystem);
            out.push(Location {
                name: if name.is_empty() { id.into() } else { name },
                path: virtual_path.clone().unwrap_or_default(),
                kind: "disk".into(),
                available: int_field(&info, "FreeSpace"),
                total: int_field(&info, "TotalSize"),
                removable: true,
                filesystem,
                device,
                mounted: false,
                usbfs: virtual_path.is_some(),
            });
        }
    }
    out
}

#[cfg(target_os = "macos")]
pub fn mount_device(device: &str) -> Result<String, String> {
    let id = device.trim_start_matches("/dev/");
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return Err(crate::i18n::t("无效设备").into());
    }
    fallback::clear_usb_fallback(device);
    let output = std::process::Command::new("diskutil")
        .args(["mount", id])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let script = format!(
            "do shell script \"/usr/sbin/diskutil mount {} \" with administrator privileges",
            id
        );
        let output = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if let Some(fallback) =
                fallback::fallback_after_native_failure(device, device, device, "", &error)
            {
                return fallback;
            }
            return Err(if error.is_empty() {
                crate::i18n::t("无法挂载该设备，系统可能不支持此文件系统").into()
            } else {
                error
            });
        }
    }
    let info = disk_info(id).ok_or(crate::i18n::t("无法读取挂载信息"))?;
    let mount = string_field(&info, "MountPoint");
    if mount.is_empty() {
        return Err(crate::i18n::t("设备未挂载，系统可能不支持此文件系统").into());
    }
    Ok(mount)
}
