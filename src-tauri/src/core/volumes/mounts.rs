//! The mount table: mounted disks with their free space, removable media that
//! is currently ejected, and the code that mounts it again.

use super::Location;
use std::collections::HashSet;
#[cfg(feature = "usbfs")]
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[cfg(feature = "usbfs")]
#[derive(Clone)]
struct FallbackMount {
    path: String,
    eject_device: String,
}

#[cfg(feature = "usbfs")]
fn usb_fallbacks() -> &'static Mutex<HashMap<String, FallbackMount>> {
    static FALLBACKS: OnceLock<Mutex<HashMap<String, FallbackMount>>> = OnceLock::new();
    FALLBACKS.get_or_init(Default::default)
}

/// Clear the fallback-provider record after ejecting or retrying a native mount.
pub fn clear_usb_fallback(device: &str) {
    #[cfg(feature = "usbfs")]
    if let Ok(mut fallbacks) = usb_fallbacks().lock() {
        fallbacks.remove(device);
    }
    #[cfg(not(feature = "usbfs"))]
    let _ = device;
}

/// The fallback view is used only after the native OS mount reports that its
/// filesystem driver is unavailable. The provider is intentionally limited to
/// Btrfs and Apple formats; local OS mounts own all other USB filesystems.
#[cfg(feature = "usbfs")]
fn fallback_mount(
    device_key: &str,
    provider_device: &str,
    eject_device: &str,
    filesystem: &str,
) -> Option<Result<String, String>> {
    let kind = crate::usbfs::detect(filesystem)
        .or_else(|| crate::usbfs::probe(provider_device))
        .filter(|kind| {
            matches!(
                kind,
                crate::usbfs::FsKind::Apfs
                    | crate::usbfs::FsKind::Hfs
                    | crate::usbfs::FsKind::Btrfs
            )
        })?;
    let path = crate::usbfs::virtual_root(provider_device, kind);
    let route =
        crate::usbfs::route(&path).ok_or_else(|| "invalid read-only USB fallback".to_string());
    Some(route.and_then(|route| {
        crate::usbfs::ensure_access(&route)?;
        usb_fallbacks()
            .lock()
            .map_err(|_| "USB fallback registry unavailable".to_string())?
            .insert(
                device_key.into(),
                FallbackMount {
                    path: path.clone(),
                    eject_device: eject_device.into(),
                },
            );
        Ok(path)
    }))
}

#[cfg(feature = "usbfs")]
fn active_fallback(device: &str) -> Option<FallbackMount> {
    usb_fallbacks().lock().ok()?.get(device).cloned()
}

#[cfg(not(feature = "usbfs"))]
fn active_fallback(_device: &str) -> Option<()> {
    None
}

pub fn usb_fallback_eject_device(device: &str) -> Option<String> {
    #[cfg(feature = "usbfs")]
    return active_fallback(device).map(|mount| mount.eject_device);
    #[cfg(not(feature = "usbfs"))]
    None
}

fn native_mount_unsupported(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "unknown filesystem",
        "unsupported filesystem",
        "filesystem not supported",
        "not supported by this system",
        "no mountable file systems",
        "wrong fs type",
        "unknown file system",
        "file system not recognized",
        "does not recognize",
    ]
    .iter()
    .any(|phrase| error.contains(phrase))
}

#[cfg(windows)]
fn windows_native_mount_unsupported(error: &std::io::Error) -> bool {
    // Windows filesystem discovery returns these errors when the volume has
    // no installed filesystem driver. Access denied is deliberately excluded:
    // ACL and device permission failures belong to Error Center.
    matches!(error.raw_os_error(), Some(1 | 50 | 1005 | 1112))
}

fn fallback_after_native_failure(
    device_key: &str,
    provider_device: &str,
    eject_device: &str,
    filesystem: &str,
    error: &str,
) -> Option<Result<String, String>> {
    if !native_mount_unsupported(error) {
        return None;
    }
    #[cfg(feature = "usbfs")]
    return fallback_mount(device_key, provider_device, eject_device, filesystem);
    #[cfg(not(feature = "usbfs"))]
    None
}

/// Mounted disks plus removable volumes that are currently unmounted.
pub(super) fn disks() -> Vec<Location> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for disk in sysinfo::Disks::new_with_refreshed_list().list() {
        let path = disk.mount_point().to_string_lossy().into_owned();
        #[cfg(target_os = "macos")]
        if !visible_mount(&path) {
            continue;
        }
        let mut name = disk.name().to_string_lossy().into_owned();
        if name.is_empty() {
            name = path.clone();
        }
        if !seen.insert((name.clone(), disk.total_space())) {
            continue;
        }
        out.push(Location {
            name,
            path,
            kind: "disk".into(),
            available: disk.available_space(),
            total: disk.total_space(),
            removable: disk.is_removable(),
            filesystem: disk.file_system().to_string_lossy().into_owned(),
            device: String::new(),
            mounted: true,
            usbfs: false,
        });
    }
    out.extend(unmounted_removable());
    out
}

#[cfg(target_os = "macos")]
fn visible_mount(path: &str) -> bool {
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn virtual_path(device: &str, filesystem: &str) -> Option<String> {
    let _ = filesystem;
    #[cfg(feature = "usbfs")]
    return active_fallback(device).map(|mount| mount.path);
    #[cfg(not(feature = "usbfs"))]
    None
}

#[cfg(target_os = "macos")]
fn unmounted_removable() -> Vec<Location> {
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
            let virtual_path = virtual_path(&device, &filesystem);
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

#[cfg(target_os = "linux")]
fn json_bool(value: &serde_json::Value, key: &str) -> bool {
    match value.get(key) {
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::String(value)) => value == "1" || value == "true",
        Some(serde_json::Value::Number(value)) => value.as_u64().unwrap_or(0) == 1,
        _ => false,
    }
}

#[cfg(target_os = "linux")]
fn unmounted_removable() -> Vec<Location> {
    let mut out = Vec::new();
    let Ok(output) = std::process::Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,PATH,TYPE,FSTYPE,MOUNTPOINT,SIZE,RM,MODEL,LABEL",
        ])
        .output()
    else {
        return out;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return out;
    };
    fn visit(items: &[serde_json::Value], removable: bool, out: &mut Vec<Location>) {
        for item in items {
            let is_removable = removable || json_bool(item, "rm");
            let kind = item
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let mount = item
                .get("mountpoint")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if kind == "part" && is_removable && mount.is_empty() {
                let filesystem = item
                    .get("fstype")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                if !filesystem.is_empty() && filesystem != "swap" {
                    let name = item
                        .get("label")
                        .and_then(|value| value.as_str())
                        .filter(|value| !value.is_empty())
                        .or_else(|| item.get("name").and_then(|value| value.as_str()))
                        .unwrap_or("USB");
                    let path = item
                        .get("path")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            format!(
                                "/dev/{}",
                                item.get("name")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("")
                            )
                        });
                    let virtual_path = virtual_path(&path, &filesystem);
                    out.push(Location {
                        name: name.into(),
                        path: virtual_path.clone().unwrap_or_default(),
                        kind: "disk".into(),
                        available: 0,
                        total: item
                            .get("size")
                            .and_then(|value| value.as_u64())
                            .unwrap_or(0),
                        removable: true,
                        filesystem,
                        device: path,
                        mounted: false,
                        usbfs: virtual_path.is_some(),
                    });
                }
            }
            if let Some(children) = item.get("children").and_then(|value| value.as_array()) {
                visit(children, is_removable, out);
            }
        }
    }
    if let Some(items) = value.get("blockdevices").and_then(|value| value.as_array()) {
        visit(items, false, &mut out);
    }
    out
}

#[cfg(all(windows, feature = "usbfs"))]
fn unmounted_removable() -> Vec<Location> {
    let script = r#"
$usb = @(Get-Disk | Where-Object { $_.BusType -eq 'USB' } | Select-Object -ExpandProperty Number)
Get-Partition | Where-Object { $usb -contains $_.DiskNumber } | ForEach-Object {
  $v = Get-Volume -Partition $_ -ErrorAction SilentlyContinue
  [pscustomobject]@{
    Name = if ($v.FileSystemLabel) { $v.FileSystemLabel } else { 'USB ' + $_.PartitionNumber }
    Device = "$($_.DiskNumber):$($_.PartitionNumber)"
    DriveLetter = $_.DriveLetter
    FileSystem = $v.FileSystem
    GptType = $_.GptType
    Size = [uint64]$_.Size
    Remaining = [uint64]$v.SizeRemaining
  }
} | ConvertTo-Json -Compress
"#;
    let mut command = std::process::Command::new("powershell");
    command.args(["-NoProfile", "-Command", script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let Ok(output) = command.output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return Vec::new();
    };
    let items = match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(_) => vec![value],
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for item in items {
        let Some(device) = item
            .get("Device")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
        else {
            continue;
        };
        let filesystem = item
            .get("FileSystem")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let has_drive_letter = item
            .get("DriveLetter")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.is_empty());
        let fallback = active_fallback(device);
        if fallback.is_none() && has_drive_letter {
            continue;
        }
        let path = fallback
            .as_ref()
            .map(|mount| mount.path.clone())
            .unwrap_or_default();
        out.push(Location {
            name: item
                .get("Name")
                .and_then(|v| v.as_str())
                .unwrap_or("USB")
                .into(),
            path,
            kind: "disk".into(),
            available: item.get("Remaining").and_then(|v| v.as_u64()).unwrap_or(0),
            total: item.get("Size").and_then(|v| v.as_u64()).unwrap_or(0),
            removable: true,
            filesystem,
            device: device.into(),
            mounted: false,
            usbfs: fallback.is_some(),
        });
    }
    out
}

#[cfg(any(
    not(any(target_os = "macos", target_os = "linux", windows)),
    all(windows, not(feature = "usbfs"))
))]
fn unmounted_removable() -> Vec<Location> {
    Vec::new()
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
    clear_usb_fallback(device);
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
                fallback_after_native_failure(device, device, device, "", &error)
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

#[cfg(target_os = "linux")]
pub fn mount_device(device: &str) -> Result<String, String> {
    if !device.starts_with("/dev/") {
        return Err(crate::i18n::t("无效设备").into());
    }
    let parse = |bytes: &[u8]| -> Option<String> {
        let text = String::from_utf8_lossy(bytes);
        text.split(" at ")
            .nth(1)
            .map(|value| value.trim().to_string())
    };
    clear_usb_fallback(device);
    let filesystem = std::process::Command::new("lsblk")
        .args(["-no", "FSTYPE", device])
        .output()
        .ok()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_lowercase()
        })
        .unwrap_or_default();
    let args = ["mount", "-b", device];
    let output = std::process::Command::new("udisksctl")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        if let Some(path) = parse(&output.stdout) {
            return Ok(path);
        }
    }
    let mut native_error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let mut pkexec_args = vec!["udisksctl"];
    pkexec_args.extend(args);
    let output = std::process::Command::new("pkexec")
        .args(&pkexec_args)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            if let Some(fallback) =
                fallback_after_native_failure(device, device, device, &filesystem, &native_error)
            {
                return fallback;
            }
            return Err(error.to_string());
        }
    };
    if output.status.success() {
        if let Some(path) = parse(&output.stdout) {
            return Ok(path);
        }
    }
    let privileged_error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !privileged_error.is_empty() {
        native_error.push('\n');
        native_error.push_str(&privileged_error);
    }
    if let Some(fallback) =
        fallback_after_native_failure(device, device, device, &filesystem, &native_error)
    {
        return fallback;
    }
    Err(if native_error.is_empty() {
        crate::i18n::t("无法挂载该设备，请确认系统支持此文件系统").into()
    } else {
        native_error
    })
}

#[cfg(windows)]
pub fn mount_device(device: &str) -> Result<String, String> {
    clear_usb_fallback(device);
    let (disk, partition) = device
        .split_once(':')
        .ok_or_else(|| crate::i18n::t("无效设备").to_string())?;
    if !disk.bytes().all(|value| value.is_ascii_digit())
        || !partition.bytes().all(|value| value.is_ascii_digit())
    {
        return Err(crate::i18n::t("无效设备").into());
    }
    let script = r#"
$ErrorActionPreference = 'Stop'
$partition = Get-Partition -DiskNumber ([int]$env:FIEX_USB_DISK) -PartitionNumber ([int]$env:FIEX_USB_PARTITION)
if (-not $partition.DriveLetter) {
  Add-PartitionAccessPath -InputObject $partition -AssignDriveLetter
  $partition = Get-Partition -DiskNumber ([int]$env:FIEX_USB_DISK) -PartitionNumber ([int]$env:FIEX_USB_PARTITION)
}
$volume = Get-Volume -Partition $partition -ErrorAction SilentlyContinue
$letter = $partition.DriveLetter
if ($letter) { $path = "$letter`:" + '\' } else { $path = '' }
[pscustomobject]@{ Path = $path; FileSystem = $volume.FileSystem } | ConvertTo-Json -Compress
"#;
    let mut command = std::process::Command::new("powershell");
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    command.env("FIEX_USB_DISK", disk);
    command.env("FIEX_USB_PARTITION", partition);
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if error.is_empty() {
            crate::i18n::t("无法使用系统驱动挂载该设备").into()
        } else {
            error
        });
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let path = value
        .get("Path")
        .and_then(serde_json::Value::as_str)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| crate::i18n::t("系统未为该设备分配挂载路径").to_string())?;
    let filesystem = value
        .get("FileSystem")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match std::fs::read_dir(path) {
        Ok(_) => Ok(path.into()),
        Err(error) => {
            if windows_native_mount_unsupported(&error) {
                let letter = path
                    .as_bytes()
                    .first()
                    .copied()
                    .filter(|value| value.is_ascii_alphabetic())
                    .map(char::from)
                    .ok_or_else(|| crate::i18n::t("系统未为该设备分配挂载路径").to_string())?;
                let raw_device = format!(r"\\.\{letter}:");
                #[cfg(feature = "usbfs")]
                if let Some(fallback) = fallback_mount(device, &raw_device, path, filesystem) {
                    return fallback;
                }
            }
            Err(error.to_string())
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub fn mount_device(_device: &str) -> Result<String, String> {
    Err(crate::i18n::t("当前系统请通过磁盘管理挂载该设备").into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_native_driver_absence_enables_usb_provider_fallback() {
        assert!(super::native_mount_unsupported(
            "unknown filesystem type 'btrfs'"
        ));
        assert!(!super::native_mount_unsupported("Permission denied"));
        assert!(!super::native_mount_unsupported("Operation not permitted"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_access_denied_never_enables_raw_provider_fallback() {
        assert!(super::windows_native_mount_unsupported(
            &std::io::Error::from_raw_os_error(1005)
        ));
        assert!(!super::windows_native_mount_unsupported(
            &std::io::Error::from_raw_os_error(5)
        ));
    }

    #[cfg(feature = "usbfs")]
    #[test]
    fn only_btrfs_and_apple_filesystems_are_provider_fallbacks() {
        for filesystem in ["btrfs", "apfs", "hfs+"] {
            assert!(super::fallback_mount(
                "test-device",
                "/dev/nonexistent-fallback-test",
                "/dev/nonexistent-fallback-test",
                filesystem,
            )
            .is_some());
        }
        assert!(super::fallback_mount(
            "test-device",
            "/dev/nonexistent-fallback-test",
            "/dev/nonexistent-fallback-test",
            "ntfs",
        )
        .is_none());
        assert!(super::fallback_mount(
            "test-device",
            "/dev/nonexistent-fallback-test",
            "/dev/nonexistent-fallback-test",
            "ext4",
        )
        .is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_hides_synthetic_apfs_volumes() {
        assert!(super::visible_mount("/"));
        assert!(super::visible_mount("/Volumes/External"));
        assert!(!super::visible_mount("/System/Volumes/Data"));
        assert!(!super::visible_mount("/System/Volumes/VM"));
    }
}
