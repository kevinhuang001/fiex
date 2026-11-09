//! Linux removable volume discovery and native mount calls.
use super::super::{super::Location, fallback};

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
pub(crate) fn unmounted_removable() -> Vec<Location> {
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
                    let virtual_path = fallback::virtual_path(&path, &filesystem);
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
    fallback::clear_usb_fallback(device);
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
    if output.status.success()
        && let Some(path) = parse(&output.stdout)
    {
        return Ok(path);
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
            if let Some(fallback) = fallback::fallback_after_native_failure(
                device,
                device,
                device,
                &filesystem,
                &native_error,
            ) {
                return fallback;
            }
            return Err(error.to_string());
        }
    };
    if output.status.success()
        && let Some(path) = parse(&output.stdout)
    {
        return Ok(path);
    }
    let privileged_error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !privileged_error.is_empty() {
        native_error.push('\n');
        native_error.push_str(&privileged_error);
    }
    if let Some(fallback) =
        fallback::fallback_after_native_failure(device, device, device, &filesystem, &native_error)
    {
        return fallback;
    }
    Err(if native_error.is_empty() {
        crate::i18n::t("无法挂载该设备，请确认系统支持此文件系统").into()
    } else {
        native_error
    })
}
