//! Platform-specific Applications enumeration and launch behavior.
mod backend;
pub(super) use backend::backend;
use std::path::Path;

/// Platform-owned classifier shared by Applications imports, exports and
/// stored user-app launch checks. Unknown or stale entries are never treated
/// as launchable.
pub(super) fn classify_host_application(path: &Path, target: Option<&str>) -> bool {
    #[cfg(not(target_os = "windows"))]
    let _ = target;
    let Ok(link) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if link.file_type().is_symlink() {
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
        {
            return target.is_some_and(|target| {
                Path::new(target).is_file() && super::is_executable_target(target)
            });
        }
        if path.is_file() && super::is_executable_target(&path.to_string_lossy()) {
            return crate::core::volumes::application_policy::executable_file(
                &path.to_string_lossy(),
            ) || !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
        }
    }

    #[cfg(target_os = "linux")]
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("desktop"))
    {
        return linux::launchable_desktop(path);
    }

    #[cfg(target_os = "macos")]
    {
        if is_application_bundle(path) {
            return true;
        }
        if is_executable_finder_alias(path) {
            return true;
        }
    }

    crate::core::volumes::application_policy::executable_file(&path.to_string_lossy())
}

pub(super) fn is_user_application(path: &Path) -> bool {
    let target = crate::core::files::entry(path)
        .ok()
        .and_then(|entry| entry.link_target);
    classify_host_application(path, target.as_deref())
}

#[cfg(target_os = "macos")]
fn is_application_bundle(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        && path.is_dir()
        && path.join("Contents/Info.plist").is_file()
}

#[cfg(target_os = "macos")]
fn is_executable_finder_alias(path: &Path) -> bool {
    let Ok(kind) = std::process::Command::new("mdls")
        .args(["-raw", "-name", "kMDItemContentType"])
        .arg(path)
        .output()
    else {
        return false;
    };
    if String::from_utf8_lossy(&kind.stdout)
        .trim()
        .trim_matches('"')
        != "com.apple.alias-file"
    {
        return false;
    }
    let script = "on run argv\ntell application \"Finder\"\nset originalFile to original item of (POSIX file (item 1 of argv) as alias)\nreturn POSIX path of (originalFile as alias)\nend tell\nend run";
    let Ok(output) = std::process::Command::new("osascript")
        .args(["-e", script])
        .arg(path)
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let Ok(target) = String::from_utf8(output.stdout) else {
        return false;
    };
    let target = Path::new(target.trim());
    if target.is_dir() {
        return is_application_bundle(target);
    }
    crate::core::volumes::application_policy::executable_file(&target.to_string_lossy())
}

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
