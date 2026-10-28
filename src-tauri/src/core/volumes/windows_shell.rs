//! Hidden PowerShell bridge for shell-namespace enumeration. It runs with no
//! console window and never asks Explorer to show a folder.
#[cfg(target_os = "windows")]
pub fn run(script: &str) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut last_error = String::new();
    for executable in ["powershell.exe", "pwsh.exe"] {
        match std::process::Command::new(executable)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            Ok(output) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
            Ok(output) => last_error = String::from_utf8_lossy(&output.stderr).trim().to_string(),
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(if last_error.is_empty() {
        crate::i18n::t("无法访问系统应用列表").into()
    } else {
        last_error
    })
}
