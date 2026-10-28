//! Local shell discovery and profile-aware PTY startup.
use super::{
    shell_profile::startup_args,
    ssh::{home, start_session, SshEvent},
};
#[cfg(not(windows))]
use crate::core::shells::order_shells;
use crate::core::shells::shell_name;
use portable_pty::CommandBuilder;
use std::path::Path;
use tauri::ipc::Channel;
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellChoice {
    id: String,
    name: String,
    path: String,
}

fn shell_choices() -> Vec<ShellChoice> {
    #[cfg(not(windows))]
    let login = std::env::var("SHELL").ok();
    #[cfg(not(windows))]
    let mut listed = Vec::new();
    #[cfg(unix)]
    if let Ok(contents) = std::fs::read_to_string("/etc/shells") {
        listed.extend(
            contents
                .lines()
                .map(str::trim)
                .filter(|line| line.starts_with('/') && !line.contains('#'))
                .map(str::to_string),
        );
    }
    #[cfg(unix)]
    for name in crate::core::shells::PROFILE_SHELLS {
        if let Ok(path) = which::which(name) {
            listed.push(path.to_string_lossy().into_owned());
        }
    }
    #[cfg(not(windows))]
    let paths = {
        let mut paths = order_shells(login.as_deref(), listed);
        if paths.is_empty() {
            paths.push("/bin/sh".into());
        }
        paths
    };
    #[cfg(windows)]
    let paths = [
        "pwsh",
        "powershell",
        "cmd",
        "bash",
        "zsh",
        "fish",
        "nu",
        "elvish",
        "xonsh",
    ]
    .into_iter()
    .filter_map(|name| which::which(name).ok())
    .map(|path| path.to_string_lossy().into_owned())
    .collect::<Vec<_>>();
    paths
        .into_iter()
        .map(|path| {
            let name = shell_name(&path);
            ShellChoice {
                id: path.clone(),
                name,
                path,
            }
        })
        .collect()
}

#[tauri::command]
pub(crate) fn detect_shells() -> Vec<ShellChoice> {
    shell_choices()
}

/// Local integrated terminal. Reuses the same PTY/session/event pipeline as SSH.
#[tauri::command]
pub(crate) async fn local_terminal_open(
    cwd: Option<String>,
    shell: Option<String>,
    cols: u16,
    rows: u16,
    on_event: Channel<SshEvent>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shell = shell
            .filter(|value| !value.is_empty() && !value.contains(['\0', '\n', '\r']))
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    "powershell.exe".to_string()
                } else {
                    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into())
                }
            });
        let mut command = CommandBuilder::new(&shell);
        for arg in startup_args(&shell) {
            command.arg(arg);
        }
        let directory = cwd
            .filter(|value| !value.contains("://") && Path::new(value).is_dir())
            .unwrap_or_else(home);
        command.cwd(directory);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        start_session(command, cols, rows, on_event)
    })
    .await
    .map_err(|error| error.to_string())?
}
