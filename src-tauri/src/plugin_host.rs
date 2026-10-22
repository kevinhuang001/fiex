//! Transitional native plugin host. A missing sidecar is not the same as a
//! runtime error: callers may use the legacy bridge only while migrating.
use crate::plugin_sdk;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::BufReader,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Mutex, OnceLock},
};

struct Running {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}
impl Running {
    fn rpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        plugin_sdk::write_frame(&mut self.stdin, &json!({
            "jsonrpc":"2.0","id":id,"method":method,"params":params
        })).map_err(|e| e.to_string())?;
        let response = plugin_sdk::read_frame(&mut self.stdout)
            .map_err(|e| e.to_string())?.ok_or("plugin exited during request")?;
        if response.get("id").and_then(Value::as_u64) != Some(id) {
            return Err("plugin response ID mismatch".into());
        }
        if let Some(error) = response.get("error") {
            return Err(format!("plugin error: {error}"));
        }
        response.get("result").cloned().ok_or_else(|| "plugin response missing result".into())
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct PluginHost {
    children: Mutex<HashMap<String, Running>>,
}
impl PluginHost {
    fn executable(plugin_id: &str) -> Result<std::path::PathBuf, String> {
        let name = match plugin_id {
            "org.fiex.webdav" => "fiex-plugin-webdav",
            "org.fiex.smb" => "fiex-plugin-smb",
            "org.fiex.git" => "fiex-plugin-git",
            _ => return Err("plugin is not installed".into()),
        };
        let exe = format!("{name}{}", std::env::consts::EXE_SUFFIX);
        let dir = std::env::var_os("FIEX_PLUGIN_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(std::path::Path::to_path_buf)))
            .ok_or("unable to locate plugin directory")?;
        Ok(dir.join(exe))
    }
    pub fn is_installed(&self, plugin_id: &str) -> bool {
        Self::executable(plugin_id).is_ok_and(|p| p.is_file())
    }
    pub fn invoke(&self, plugin_id: &str, method: &str, params: Value) -> Result<Value, String> {
        let mut guard = self.children.lock().map_err(|_| "plugin host lock is poisoned")?;
        if !guard.contains_key(plugin_id) {
            let path = Self::executable(plugin_id)?;
            let mut child = Command::new(&path)
                .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
                .spawn().map_err(|e| format!("plugin start failed: {e}"))?;
            let stdin = child.stdin.take().ok_or("plugin stdin unavailable")?;
            let stdout = child.stdout.take().ok_or("plugin stdout unavailable")?;
            let mut running = Running { child, stdin, stdout: BufReader::new(stdout), next_id: 0 };
            let result = running.rpc("plugin.initialize", json!({"protocolVersion":plugin_sdk::PROTOCOL_VERSION}))?;
            if result.get("pluginId").and_then(Value::as_str) != Some(plugin_id) {
                return Err("plugin identity mismatch".into());
            }
            guard.insert(plugin_id.into(), running);
        }
        let response = guard.get_mut(plugin_id).ok_or("plugin unavailable")?.rpc(method, params);
        if response.is_err() { guard.remove(plugin_id); }
        response
    }
}
pub fn global() -> &'static PluginHost {
    static INSTANCE: OnceLock<PluginHost> = OnceLock::new();
    INSTANCE.get_or_init(PluginHost::default)
}
