//! The AppContainer launcher requires all three standard handles, even when
//! Fiex was opened from Explorer and has no console or inherited stderr.
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
};

pub(super) fn spawn(executable: &Path, payload: &str) -> Result<Child, String> {
    // Arapuca skips invalid standard handles and only sets STARTF_USESTDHANDLES
    // when it has all three. Never inherit the GUI application's absent stderr:
    // it would also prevent the plugin's stdin/stdout IPC from being bound.
    let mut child = Command::new(executable)
        .arg("--fiex-plugin-sandbox")
        .arg(payload)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("plugin sandbox launcher failed: {e}"))?;
    let mut stderr = child.stderr.take().expect("piped launcher stderr");
    if let Err(error) = std::thread::Builder::new()
        .name("fiex-plugin-stderr".into())
        .spawn(move || {
            let mut buffer = [0; 4096];
            loop {
                match stderr.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        // A GUI host may have no writable stderr. Keep draining
                        // regardless, so plugin diagnostics cannot fill the pipe
                        // and stall JSON-RPC. Never mix them into protocol stdout.
                        let _ = std::io::stderr().write_all(&buffer[..count]);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        })
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("plugin stderr reader unavailable: {error}"));
    }
    Ok(child)
}
