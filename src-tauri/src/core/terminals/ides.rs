use super::*;

#[derive(Serialize)]
pub struct Ide {
    pub id: String,
    pub name: String,
}

pub(super) const IDES: &[(&str, &str, &str, &str)] = &[
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

#[cfg(target_os = "windows")]
fn windows_ide_executable(id: &str) -> Option<std::path::PathBuf> {
    use winreg::{
        RegKey,
        enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
    };

    let executable = match id {
        "vscode" => "Code.exe",
        "vscode-insiders" => "Code - Insiders.exe",
        "cursor" => "Cursor.exe",
        "windsurf" => "Windsurf.exe",
        "zed" => "zed.exe",
        "sublime" => "sublime_text.exe",
        "idea" => "idea64.exe",
        "webstorm" => "webstorm64.exe",
        "pycharm" => "pycharm64.exe",
        "android-studio" => "studio64.exe",
        _ => return None,
    };
    if let Ok(path) = which::which(executable) {
        return path.is_file().then_some(path);
    }

    let app_paths = format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{executable}");
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        if let Ok(key) = RegKey::predef(hive).open_subkey(&app_paths)
            && let Ok(value) = key.get_value::<String, _>("")
        {
            let path = std::path::PathBuf::from(value.trim().trim_matches('"'));
            if path.is_file() {
                return Some(path);
            }
        }
    }

    let local = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
    let program_files = ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    if let Some(local) = local {
        let relative = match id {
            "vscode" => "Programs/Microsoft VS Code/Code.exe",
            "vscode-insiders" => "Programs/Microsoft VS Code Insiders/Code - Insiders.exe",
            "cursor" => "Programs/Cursor/Cursor.exe",
            "windsurf" => "Programs/Windsurf/Windsurf.exe",
            _ => "",
        };
        if !relative.is_empty() {
            candidates.push(local.join(relative));
        }
    }
    let relative = match id {
        "vscode" => "Microsoft VS Code/Code.exe",
        "vscode-insiders" => "Microsoft VS Code Insiders/Code - Insiders.exe",
        "sublime" => "Sublime Text/sublime_text.exe",
        "idea" => "JetBrains/IntelliJ IDEA/bin/idea64.exe",
        "webstorm" => "JetBrains/WebStorm/bin/webstorm64.exe",
        "pycharm" => "JetBrains/PyCharm/bin/pycharm64.exe",
        "android-studio" => "Android Studio/bin/studio64.exe",
        _ => "",
    };
    if !relative.is_empty() {
        candidates.extend(program_files.into_iter().map(|root| root.join(relative)));
    }
    candidates.into_iter().find(|path| path.is_file())
}

pub fn detect_ides() -> Vec<Ide> {
    let mut out = Vec::new();
    for (id, name, _cli, _app) in IDES {
        #[cfg(target_os = "windows")]
        let installed = windows_ide_executable(id).is_some();
        #[cfg(target_os = "macos")]
        let installed = {
            which::which(_cli).is_ok()
                || std::path::Path::new(&format!("/Applications/{_app}.app")).exists()
                || dirs::home_dir()
                    .is_some_and(|home| home.join(format!("Applications/{_app}.app")).exists())
        };
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let installed = which::which(_cli).is_ok();
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
    crate::plugins::mounts::reject_local(path)?;
    let path = dunce::canonicalize(path).map_err(|error| error.to_string())?;
    let (id, _app) = IDES
        .iter()
        .find(|(candidate, _, _, _)| *candidate == id)
        .map(|(id, _, _, _app)| (*id, *_app))
        .unwrap_or(("vscode", "Visual Studio Code"));
    #[cfg(any(target_os = "macos", target_os = "linux"))]
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
        let executable = windows_ide_executable(id)
            .ok_or_else(|| crate::i18n::t("所选 IDE 未安装").to_string())?;
        Command::new(executable)
            .arg(&path)
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
