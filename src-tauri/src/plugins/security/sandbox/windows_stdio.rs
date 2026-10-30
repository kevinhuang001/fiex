//! The AppContainer launcher requires all three standard handles, even when
//! Fiex was opened from Explorer and has no console or inherited stderr.
use std::{
    os::windows::process::CommandExt,
    path::Path,
    process::{Child, Command, Stdio},
};

pub(super) fn spawn(executable: &Path, payload: &str) -> Result<Child, String> {
    // Arapuca skips invalid standard handles and only sets STARTF_USESTDHANDLES
    // when it has all three. Never inherit the GUI application's absent stderr:
    // it would also prevent the plugin's stdin/stdout IPC from being bound.
    let mut command = Command::new(executable);
    command
        .arg("--fiex-plugin-sandbox")
        .arg(payload)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The GUI host has no console; never allocate one for the plugin.
        .creation_flags(0x08000000);
    // stderr stays piped and is drained by the unified plugin LogCenter.
    // The wrapper and confined plugin share the same inherited stderr handle.
    command
        .spawn()
        .map_err(|e| format!("plugin sandbox launcher failed: {e}"))
}
