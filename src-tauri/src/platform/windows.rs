//! Windows volume helpers shared by the desktop and USB adapters.

use std::os::windows::process::CommandExt;

/// Do not flash a console window for the short-lived Shell helper.
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Drive letter for a Windows mount path such as `E:`, `E:\` or `E:\folder`.
/// Volume-GUID paths have no letter and return `None`.
pub(crate) fn drive_letter(path: &str) -> Option<char> {
    let bytes = path.trim().as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        Some(bytes[0] as char)
    } else {
        None
    }
}

/// Eject a mounted volume through the same Shell verb Explorer exposes.
pub(crate) fn eject_path(path: &str) -> Result<(), String> {
    let drive = drive_letter(path)
        .ok_or_else(|| crate::i18n::t("请先在系统中挂载该设备再弹出").to_string())?;
    eject_drive(drive)
}

fn eject_drive(drive: char) -> Result<(), String> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$drive = $env:FIEX_EJECT_DRIVE
$shell = New-Object -ComObject Shell.Application
$item = $shell.Namespace(17).ParseName("${drive}:\")
if ($null -eq $item) { $item = $shell.Namespace(17).ParseName("${drive}:") }
if ($null -eq $item) { exit 2 }
$verb = @($item.Verbs()) | Where-Object {
  ($_.Name -replace '&', '') -match '^(Eject|弹出|退出)'
} | Select-Object -First 1
if ($null -ne $verb) { $verb.DoIt() } else { $item.InvokeVerb('Eject') }
for ($i = 0; $i -lt 40; $i++) {
  if (-not (Test-Path "${drive}:\")) { exit 0 }
  Start-Sleep -Milliseconds 250
}
exit 3
"#;
    let mut command = std::process::Command::new("powershell");
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    command.env("FIEX_EJECT_DRIVE", drive.to_string());
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    if output.status.code() == Some(3) {
        return Err(crate::i18n::t("无法弹出该设备，请确认没有程序正在使用它").into());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = stderr.trim();
    Err(if message.is_empty() {
        crate::i18n::t("无法弹出该设备").into()
    } else {
        message.into()
    })
}
