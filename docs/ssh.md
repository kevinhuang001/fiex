# Remote terminals

SSH connections are implemented by plugins. Core resolves the owning plugin through `connection.open` and hosts its package-backed PluginView. Local shell PTYs are a separate Core feature under `desktop/terminal.rs`. No official plugin ID, SSH connection schema, remote PTY bridge or OpenSSH connection fallback is registered in Core.

Existing builtin SSH records must be recreated using an installed connection plugin. Core does not silently transfer credentials.
