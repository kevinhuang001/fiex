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
        ("Tabby", "Tabby"),
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
        ("tabby.exe", "Tabby"),
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
        ("tabby", "Tabby"),
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
#[cfg(target_os = "macos")]
fn shell_escape(path: &std::path::Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
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
            "Tabby" => {
                if which::which("tabby").is_ok() {
                    Command::new("tabby")
                        .arg("open")
                        .arg(&dir)
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
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
        Err("无法打开“完全磁盘访问权限”设置，请手动打开系统设置".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("完全磁盘访问权限设置仅在 macOS 上提供".into())
    }
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

#[derive(Serialize)]
pub struct Ide {
    pub id: String,
    pub name: String,
}

const IDES: &[(&str, &str, &str, &str)] = &[
    ("vscode", "VS Code", "code", "Visual Studio Code"),
    (
        "vscode-insiders",
        "VS Code Insiders",
        "code-insiders",
        "Visual Studio Code - Insiders",
    ),
    ("cursor", "Cursor", "cursor", "Cursor"),
    ("windsurf", "Windsurf", "windsurf", "Windsurf"),
    ("zed", "Zed", "zed", "Zed"),
    ("sublime", "Sublime Text", "subl", "Sublime Text"),
    ("idea", "IntelliJ IDEA", "idea", "IntelliJ IDEA"),
    ("webstorm", "WebStorm", "webstorm", "WebStorm"),
    ("pycharm", "PyCharm", "pycharm", "PyCharm"),
    (
        "android-studio",
        "Android Studio",
        "studio",
        "Android Studio",
    ),
    ("xcode", "Xcode", "xed", "Xcode"),
];

pub fn detect_ides() -> Vec<Ide> {
    let mut out = Vec::new();
    for (id, name, cli, _app) in IDES {
        #[allow(unused_mut)]
        let mut installed = which::which(cli).is_ok();
        #[cfg(target_os = "macos")]
        {
            installed = installed
                || std::path::Path::new(&format!("/Applications/{_app}.app")).exists()
                || dirs::home_dir()
                    .is_some_and(|home| home.join(format!("Applications/{_app}.app")).exists());
        }
        #[cfg(target_os = "windows")]
        {
            installed = installed
                || which::which(format!("{cli}.cmd")).is_ok()
                || which::which(format!("{cli}.exe")).is_ok();
        }
        if installed {
            out.push(Ide {
                id: (*id).into(),
                name: (*name).into(),
            });
        }
    }
    out
}

pub fn launch_ide(path: &str, id: &str) -> Result<(), String> {
    let path = dunce::canonicalize(path).map_err(|error| error.to_string())?;
    let (id, _app) = IDES
        .iter()
        .find(|(candidate, _, _, _)| *candidate == id)
        .map(|(id, _, _, _app)| (*id, *_app))
        .unwrap_or(("vscode", "Visual Studio Code"));
    let (_, _, cli, _) = IDES
        .iter()
        .find(|(candidate, _, _, _)| *candidate == id)
        .copied()
        .unwrap_or(("vscode", "VS Code", "code", "Visual Studio Code"));
    #[cfg(target_os = "macos")]
    {
        if which::which(cli).is_ok() {
            Command::new(cli)
                .arg(&path)
                .spawn()
                .map_err(|error| error.to_string())?;
        } else {
            Command::new("open")
                .args(["-a", _app])
                .arg(&path)
                .spawn()
                .map_err(|error| error.to_string())?;
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .args(["/C", cli])
            .arg(&path)
            .creation_flags(0x00000010)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new(cli)
            .arg(&path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
