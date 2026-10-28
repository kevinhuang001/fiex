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
    crate::plugins::mounts::reject_local(path)?;
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
