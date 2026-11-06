//! Windows volume helpers shared by the desktop and USB adapters.

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
    crate::core::volumes::windows_native::eject_drive(drive)
        .map_err(|_| crate::i18n::t("无法弹出该设备，请确认没有程序正在使用它").to_string())
}
