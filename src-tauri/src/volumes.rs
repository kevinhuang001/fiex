use serde::Serialize;
use std::{collections::HashSet, path::PathBuf};

#[derive(Serialize)]
pub struct Location {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub available: u64,
    pub total: u64,
    pub removable: bool,
    pub filesystem: String,
    pub device: String,
    pub mounted: bool,
    pub usbfs: bool,
}
#[cfg(target_os = "macos")]
fn visible_mount(path: &str) -> bool {
    // APFS exposes the data volume and several synthetic volumes from the same
    // container. Only the root and user-mounted volumes should be listed.
    !path.starts_with("/System/Volumes/") && (path == "/" || path.starts_with("/Volumes/"))
}

pub fn locations() -> Vec<Location> {
    let mut out = Vec::new();
    let named: [(&str, Option<PathBuf>); 7] = [
        ("个人文件夹", dirs::home_dir()),
        ("桌面", dirs::desktop_dir()),
        ("下载", dirs::download_dir()),
        ("文稿", dirs::document_dir()),
        ("图片", dirs::picture_dir()),
        ("音乐", dirs::audio_dir()),
        ("视频", dirs::video_dir()),
    ];
    for (name, path) in named {
        if let Some(path) = path.filter(|p| p.exists()) {
            out.push(Location {
                name: name.into(),
                path: path.to_string_lossy().into_owned(),
                kind: "favorite".into(),
                available: 0,
                total: 0,
                removable: false,
                filesystem: String::new(),
                device: String::new(),
                mounted: true,
                usbfs: false,
            });
        }
    }
    // macOS has a real Applications folder. Windows and Linux expose installed
    // applications through shell/desktop services, so those entries only open
    // the platform UI instead of pretending to be a browsable directory.
    let applications = if cfg!(target_os = "macos") {
        let path = PathBuf::from("/Applications");
        path.exists().then_some(path)
    } else {
        None
    };
    out.push(Location {
        name: "应用程序".into(),
        path: applications
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        kind: "applications".into(),
        available: 0,
        total: 0,
        removable: false,
        filesystem: String::new(),
        device: String::new(),
        mounted: true,
        usbfs: false,
    });
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

#[allow(dead_code)]
fn virtual_path(device: &str, filesystem: &str) -> Option<String> {
    #[cfg(feature = "usbfs")]
    {
        crate::usbfs::detect(filesystem).map(|kind| crate::usbfs::virtual_root(device, kind))
    }
    #[cfg(not(feature = "usbfs"))]
    {
        let _ = (device, filesystem);
        None
    }
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
Get-Partition | Where-Object { $usb -contains $_.DiskNumber -and -not $_.DriveLetter } | ForEach-Object {
  $v = Get-Volume -Partition $_ -ErrorAction SilentlyContinue
  [pscustomobject]@{
    Name = if ($v.FileSystemLabel) { $v.FileSystemLabel } else { 'USB ' + $_.PartitionNumber }
    Path = ($_.AccessPaths | Select-Object -First 1)
    FileSystem = $v.FileSystem
    GptType = $_.GptType
    Size = [uint64]$_.Size
    Remaining = [uint64]$v.SizeRemaining
  }
} | ConvertTo-Json -Compress
"#;
    let Ok(output) = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
    else {
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
            .get("Path")
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
        let Some(kind) = crate::usbfs::detect(&filesystem).or_else(|| crate::usbfs::probe(device))
        else {
            continue;
        };
        out.push(Location {
            name: item
                .get("Name")
                .and_then(|v| v.as_str())
                .unwrap_or("USB")
                .into(),
            path: crate::usbfs::virtual_root(device, kind),
            kind: "disk".into(),
            available: item.get("Remaining").and_then(|v| v.as_u64()).unwrap_or(0),
            total: item.get("Size").and_then(|v| v.as_u64()).unwrap_or(0),
            removable: true,
            filesystem,
            device: device.into(),
            mounted: false,
            usbfs: true,
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
        return Err("无效设备".into());
    }
    let output = std::process::Command::new("diskutil")
        .args(["mount", "readOnly", id])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let script = format!(
            "do shell script \"/usr/sbin/diskutil mount readOnly {} \" with administrator privileges",
            id
        );
        let output = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(if error.trim().is_empty() {
                "无法挂载该设备，系统可能不支持此文件系统".into()
            } else {
                error.trim().into()
            });
        }
    }
    let info = disk_info(id).ok_or("无法读取挂载信息")?;
    let mount = string_field(&info, "MountPoint");
    if mount.is_empty() {
        return Err("设备未挂载，系统可能不支持此文件系统".into());
    }
    Ok(mount)
}

#[cfg(target_os = "linux")]
pub fn mount_device(device: &str) -> Result<String, String> {
    if !device.starts_with("/dev/") {
        return Err("无效设备".into());
    }
    let parse = |bytes: &[u8]| -> Option<String> {
        let text = String::from_utf8_lossy(bytes);
        text.split(" at ")
            .nth(1)
            .map(|value| value.trim().to_string())
    };
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
    let writable = matches!(
        filesystem.as_str(),
        "ext2" | "ext3" | "ext4" | "ntfs" | "ntfs3"
    );
    let mut args = vec!["mount", "-b", device];
    if !writable {
        args.extend(["-o", "ro"]);
    }
    let output = std::process::Command::new("udisksctl")
        .args(&args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        if let Some(path) = parse(&output.stdout) {
            return Ok(path);
        }
    }
    let mut pkexec_args = vec!["udisksctl"];
    pkexec_args.extend(args);
    let output = std::process::Command::new("pkexec")
        .args(&pkexec_args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        if let Some(path) = parse(&output.stdout) {
            return Ok(path);
        }
    }
    Err("无法挂载该设备，请确认系统支持此文件系统".into())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn mount_device(_device: &str) -> Result<String, String> {
    Err("当前系统请通过磁盘管理挂载该设备".into())
}

/// The user's Trash can be browsed like any other folder. Windows exposes the
/// Recycle Bin through a shell namespace instead of a directory, so it keeps
/// the system opener as a fallback.
pub fn trash_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|home| home.join(".Trash"))
            .filter(|path| path.is_dir())
    }
    #[cfg(target_os = "linux")]
    {
        dirs::data_dir()
            .map(|data| data.join("Trash/files"))
            .filter(|path| path.is_dir())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_hides_synthetic_apfs_volumes() {
        assert!(super::visible_mount("/"));
        assert!(super::visible_mount("/Volumes/External"));
        assert!(!super::visible_mount("/System/Volumes/Data"));
        assert!(!super::visible_mount("/System/Volumes/VM"));
    }
}
