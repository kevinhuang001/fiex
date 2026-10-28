use super::Terminal;

pub fn detect() -> Vec<Terminal> {
    let mut out = vec![Terminal {
        id: "default".into(),
        name: crate::i18n::t("系统默认终端").into(),
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
        ("x-terminal-emulator", crate::i18n::t("系统终端")),
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
