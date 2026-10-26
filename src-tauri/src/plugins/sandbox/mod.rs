//! OS-level plugin confinement.
//!
//! Desktop builds use Landlock + seccomp on Linux, a deny-default Seatbelt
//! profile on macOS, and AppContainer + a Job Object on Windows. Fiex refuses to launch a plugin if the sandbox cannot be applied.
//!
//! A failed sandbox setup is fatal: a plugin is never launched unsandboxed
//! when the host believes a policy is active.
//!
//! The `rules` module builds the platform policy (filesystem roots, plugin
//! environment and Landlock/seccomp/Seatbelt rules); the `spawn` module launches
//! the confined child, including the re-executed wrapper path.
mod rules;
mod spawn;
#[cfg(windows)]
mod windows_stdio;

pub use rules::Policy;
pub use spawn::{run_wrapper_if_requested, spawn, KillHandle, SandboxChild};
