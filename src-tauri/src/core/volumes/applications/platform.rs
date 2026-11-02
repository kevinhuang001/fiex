//! Platform-specific Applications enumeration and launch behavior.
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(all(test, target_os = "linux"))]
pub(in crate::core::volumes::applications) use linux::desktop_entry;
#[cfg(target_os = "linux")]
pub(in crate::core::volumes::applications) use linux::{platform_list, platform_open};
#[cfg(target_os = "macos")]
pub(in crate::core::volumes::applications) use macos::{platform_list, platform_open};
#[cfg(all(test, target_os = "windows"))]
pub(in crate::core::volumes::applications) use windows::resolve_windows_executable;
#[cfg(target_os = "windows")]
pub(in crate::core::volumes::applications) use windows::{platform_list, platform_open};

pub(super) fn open_stored(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if crate::core::volumes::application_policy::executable_file(&path.to_string_lossy()) {
            return std::process::Command::new(path)
                .spawn()
                .map(|_| ())
                .map_err(|error| error.to_string());
        }
        return crate::core::terminals::open_path(&path.to_string_lossy());
    }
    #[cfg(target_os = "linux")]
    {
        if path.extension().and_then(|value| value.to_str()) == Some("desktop") {
            return linux::launch_desktop(path);
        }
        if crate::core::volumes::application_policy::executable_file(&path.to_string_lossy()) {
            return std::process::Command::new(path)
                .spawn()
                .map(|_| ())
                .map_err(|error| error.to_string());
        }
        return open::that_detached(path).map_err(|error| error.to_string());
    }
    #[cfg(target_os = "windows")]
    {
        // ShellExecute keeps the launch detached and never allocates a console;
        // `cmd /C start` used to flash a console window for GUI applications.
        return open::that_detached(path).map_err(|error| error.to_string());
    }
    #[allow(unreachable_code)]
    Err(crate::i18n::t("无法启动应用").into())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub(super) fn platform_list() -> Result<Vec<crate::core::files::Entry>, String> {
    Ok(Vec::new())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub(super) fn platform_open(_encoded: &str) -> Result<(), String> {
    Err(crate::i18n::t("无法启动应用").into())
}
