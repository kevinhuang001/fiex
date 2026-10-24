//! Plugin child-process lifecycle, host method dispatch and JSON-RPC transport.
//!
//! `spawn` owns command execution (`exec`/`process.run`) and process teardown,
//! `rpc` owns the JSON-RPC framing, timeouts and `host.*` method dispatch, and
//! `host` owns the per-plugin child pool and the active directory.
mod host;
mod rpc;
mod spawn;

pub use host::{active_path, global, set_active_directory, PluginHost};
pub use spawn::run_command;
