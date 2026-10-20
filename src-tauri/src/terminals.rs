use serde::Serialize;
use std::{path::PathBuf, process::Command};
#[derive(Serialize)]
pub struct Terminal {
    pub id: String,
    pub name: String,
}
pub fn detect() -> Vec<Terminal> {
    let mut out = vec![Terminal {
        id: "default".into(),
        name: "系统默认终端".into(),
    }];
    #[cfg(target_os = "macos")]
    for (id, name) in [
        ("Terminal", "Terminal"),
        ("iTerm", "iTerm2"),
        ("Warp", "Warp"),
        ("Alacritty", "Alacritty"),
        ("kitty", "kitty"),
        ("Ghostty", "Ghostty"),
        ("WezTerm", "WezTerm"),
    ] {
        if std::path::Path::new(&format!("/Applications/{id}.app")).exists()
            || std::path::Path::new(&format!("/System/Applications/Utilities/{id}.app")).exists()
            || dirs::home_dir().is_some_and(|p| p.join(format!("Applications/{id}.app")).exists())
        {
            out.push(Terminal {
                id: id.into(),
                name: name.into(),
            });
        }
    }
    #[cfg(target_os = "windows")]
    for (id, name) in [
        ("wt.exe", "Windows Terminal"),
        ("pwsh.exe", "PowerShell 7"),
        ("powershell.exe", "Windows PowerShell"),
        ("cmd.exe", "Command Prompt"),
    ] {
        if which::which(id).is_ok() {
            out.push(Terminal {
                id: id.into(),
                name: name.into(),
            });
        }
    }
    #[cfg(target_os = "linux")]
    for (id, name) in [
        ("x-terminal-emulator", "系统终端"),
        ("kgx", "Console"),
        ("gnome-terminal", "GNOME Terminal"),
        ("konsole", "Konsole"),
        ("xfce4-terminal", "Xfce Terminal"),
        ("alacritty", "Alacritty"),
        ("kitty", "kitty"),
        ("ghostty", "Ghostty"),
        ("wezterm", "WezTerm"),
        ("xterm", "XTerm"),
    ] {
        if which::which(id).is_ok() {
            out.push(Terminal {
                id: id.into(),
                name: name.into(),
            });
        }
    }
    out
}
pub fn launch(path: &str, id: &str) -> Result<(), String> {
    let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
    let dir: PathBuf = if path.is_dir() {
        path
    } else {
        path.parent().ok_or("没有父目录")?.into()
    };
    if !detect().iter().any(|t| t.id == id) {
        return Err("所选终端未安装".into());
    }
    #[cfg(target_os = "macos")]
    {
        let app = if id == "default" { "Terminal" } else { id };
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
                .ok_or("未发现可用终端，请安装 GNOME Terminal、Konsole 等终端")?
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
    open::that_detached(path).map_err(|e| e.to_string())
}
pub fn reveal(path: &str) -> Result<(), String> {
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
        open::that_detached(std::path::Path::new(path).parent().ok_or("没有父目录")?)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn open_trash() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(dirs::home_dir().ok_or("无法获取个人文件夹")?.join(".Trash"))
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
