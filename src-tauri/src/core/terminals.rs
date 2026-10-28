use serde::Serialize;
use std::{path::PathBuf, process::Command};

mod ides;
pub use ides::*;

#[derive(Serialize)]
pub struct Terminal {
    pub id: String,
    pub name: String,
}

mod detect;

pub use detect::detect;

#[cfg(target_os = "macos")]
fn shell_escape(path: &std::path::Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

pub fn launch(path: &str, id: &str) -> Result<(), String> {
    crate::plugins::mounts::reject_local(path)?;
    let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
    let dir: PathBuf = if path.is_dir() {
        path
    } else {
        path.parent().ok_or(crate::i18n::t("没有父目录"))?.into()
    };
    if !detect().iter().any(|t| t.id == id) {
        return Err(crate::i18n::t("所选终端未安装").into());
    }
    #[cfg(target_os = "macos")]
    {
        let app = if id == "default" { "Terminal" } else { id };
        // Terminal and iTerm need AppleScript to open a session in a folder;
        // Warp has a URL scheme and Tabby's CLI opens a tab at a directory.
        match app {
            "Terminal" => {
                let script = format!(
                    "tell application \"Terminal\"\nactivate\ndo script \"cd {}\"\nend tell",
                    shell_escape(&dir)
                );
                Command::new("osascript")
                    .args(["-e", &script])
                    .spawn()
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
            "iTerm" => {
                let script = format!(
                    "tell application \"iTerm\"\nactivate\ncreate window with default profile command \"cd {}\"\nend tell",
                    shell_escape(&dir)
                );
                Command::new("osascript")
                    .args(["-e", &script])
                    .spawn()
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
            "Warp" => {
                let target = format!(
                    "warp://action/new_tab?path={}",
                    urlencoding::encode(&dir.to_string_lossy())
                );
                Command::new("open")
                    .arg(target)
                    .spawn()
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
            "Tabby" if which::which("tabby").is_ok() => {
                Command::new("tabby")
                    .arg("open")
                    .arg(&dir)
                    .spawn()
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
            _ => {}
        }
        let mut command = Command::new("open");
        match app {
            "Ghostty" => {
                command
                    .args(["-na", app, "--args"])
                    .arg(format!("--working-directory={}", dir.display()));
            }
            "Alacritty" => {
                command
                    .args(["-na", app, "--args", "--working-directory"])
                    .arg(&dir);
            }
            "kitty" => {
                command
                    .args(["-na", app, "--args", "--directory"])
                    .arg(&dir);
            }
            "WezTerm" => {
                command
                    .args(["-na", app, "--args", "start", "--cwd"])
                    .arg(&dir);
            }
            _ => {
                command.args(["-a", app]).arg(&dir);
            }
        }
        command.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let app = if id == "default" {
            if which::which("wt.exe").is_ok() {
                "wt.exe"
            } else {
                "powershell.exe"
            }
        } else {
            id
        };
        let mut cmd = Command::new(app);
        cmd.current_dir(&dir).creation_flags(0x00000010);
        if app == "wt.exe" {
            cmd.arg("-d").arg(&dir);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        let terminals = detect();
        let app = if id == "default" {
            terminals
                .get(1)
                .ok_or(crate::i18n::t(
                    "未发现可用终端，请安装 GNOME Terminal、Konsole 等终端",
                ))?
                .id
                .as_str()
        } else {
            id
        };
        let mut cmd = Command::new(app);
        cmd.current_dir(&dir);
        match app {
            "gnome-terminal" | "xfce4-terminal" | "ghostty" => {
                cmd.arg(format!("--working-directory={}", dir.display()));
            }
            "kgx" => {
                cmd.arg("--working-directory").arg(&dir);
            }
            "konsole" => {
                cmd.arg("--workdir").arg(&dir);
            }
            "kitty" => {
                cmd.arg("--directory").arg(&dir);
            }
            "alacritty" => {
                cmd.arg("--working-directory").arg(&dir);
            }
            "wezterm" => {
                cmd.args(["start", "--cwd"]).arg(&dir);
            }
            _ => {}
        }
        cmd.spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn open_path(path: &str) -> Result<(), String> {
    crate::plugins::mounts::reject_local(path)?;
    open::that_detached(path).map_err(|e| e.to_string())
}

pub fn reveal(path: &str) -> Result<(), String> {
    crate::plugins::mounts::reject_local(path)?;
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg(format!("/select,{path}"))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        open::that_detached(
            std::path::Path::new(path)
                .parent()
                .ok_or(crate::i18n::t("没有父目录"))?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Opens the platform's installed-applications UI. macOS has a real folder and
/// is handled in-app; Windows/Linux use this command because there is no folder
/// with the same semantics.
pub fn open_applications() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("/Applications")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg("shell:AppsFolder")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        for app in [
            "gnome-software",
            "plasma-discover",
            "pamac-manager",
            "mintinstall",
            "bauh",
            "octopi",
        ] {
            if which::which(app).is_ok() {
                Command::new(app).spawn().map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
        let applications = PathBuf::from("/usr/share/applications");
        let fallback = if applications.is_dir() {
            applications
        } else {
            dirs::data_dir()
                .map(|dir| dir.join("applications"))
                .unwrap_or(applications)
        };
        open::that_detached(fallback).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn open_full_disk_access() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        for url in [
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AllFiles",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
        ] {
            if Command::new("open")
                .arg(url)
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
            {
                return Ok(());
            }
        }
        Err(crate::i18n::t("无法打开“完全磁盘访问权限”设置，请手动打开系统设置").into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(crate::i18n::t("完全磁盘访问权限设置仅在 macOS 上提供").into())
    }
}

pub fn open_trash() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(
                dirs::home_dir()
                    .ok_or(crate::i18n::t("无法获取个人文件夹"))?
                    .join(".Trash"),
            )
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg("shell:RecycleBinFolder")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        open::that_detached("trash:///").map_err(|e| e.to_string())?;
    }
    Ok(())
}
