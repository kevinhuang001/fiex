//! Sandbox policy construction.
//!
//! This module owns everything a plugin's confinement policy is made of: the
//! canonical filesystem roots, the environment handed to the child, and the
//! Landlock/seccomp/Arapuca rules applied before the plugin starts.

use std::path::Path;

#[derive(Clone, Debug)]
pub struct Policy {
    pub executable: std::path::PathBuf,
    pub install_dir: std::path::PathBuf,
    pub read_roots: Vec<std::path::PathBuf>,
    pub write_roots: Vec<std::path::PathBuf>,
    pub network: bool,
}

fn system_read_roots() -> Vec<std::path::PathBuf> {
    #[cfg(unix)]
    {
        let canonical = |path: &Path| dunce::canonicalize(path).unwrap_or_else(|_| path.into());
        [
            "/usr/lib",
            "/lib",
            "/lib64",
            "/etc/ld.so.cache",
            "/dev/urandom",
            "/dev/null",
        ]
        .iter()
        .filter(|path| Path::new(path).exists())
        .map(|path| canonical(Path::new(path)))
        .collect()
    }
    #[cfg(windows)]
    {
        // AppContainer grants read/execute to System32 and Program Files
        // through the implicit ALL APPLICATION PACKAGES SID. Adding those
        // trees as explicit DACL roots both is unnecessary and can stall
        // CreateProcess while Windows propagates inheritance.
        Vec::new()
    }
}

impl Policy {
    pub fn new(
        executable: &Path,
        install_dir: &Path,
        grant: &crate::plugin_permissions::PluginGrant,
        declared: &std::collections::BTreeSet<String>,
    ) -> Self {
        let canonical = |path: &Path| dunce::canonicalize(path).unwrap_or_else(|_| path.into());
        let executable = canonical(executable);
        let install_dir = canonical(install_dir);
        let mut read_roots = vec![executable.clone()];
        read_roots.extend(system_read_roots());
        let mut write_roots = Vec::new();
        let _ = (grant, declared);
        read_roots.sort();
        read_roots.dedup();
        write_roots.sort();
        write_roots.dedup();
        Self {
            executable,
            install_dir,
            read_roots,
            write_roots,
            network: false,
        }
    }
}

/// Arapuca permits ordinary fork/clone and System V IPC for general workloads.
/// Plugins only need threads and inherited pipes, so tighten those before exec.
#[cfg(target_os = "linux")]
pub(super) fn restrict_plugin_process_creation() -> Result<(), String> {
    let instruction = |code, jt, jf, k| libc::sock_filter { code, jt, jf, k };
    let mut code = vec![instruction(0x20, 0, 0, 0)]; // seccomp_data.nr
    let denied = [
        libc::SYS_shmget,
        libc::SYS_shmat,
        libc::SYS_shmctl,
        libc::SYS_semget,
        libc::SYS_semop,
        libc::SYS_semctl,
        libc::SYS_msgget,
        libc::SYS_msgsnd,
        libc::SYS_msgrcv,
        libc::SYS_msgctl,
        libc::SYS_kill,
    ];
    for number in denied {
        code.push(instruction(0x15, 0, 1, number as u32));
        code.push(instruction(0x06, 0, 0, 0x00050000 | libc::EPERM as u32));
    }
    #[cfg(target_arch = "x86_64")]
    for number in [libc::SYS_fork, libc::SYS_vfork] {
        code.push(instruction(0x15, 0, 1, number as u32));
        code.push(instruction(0x06, 0, 0, 0x00050000 | libc::EPERM as u32));
    }
    code.extend([
        instruction(0x15, 0, 3, libc::SYS_clone as u32),
        instruction(0x20, 0, 0, 16), // low word of clone flags
        instruction(0x45, 1, 0, libc::CLONE_THREAD as u32),
        instruction(0x06, 0, 0, 0x00050000 | libc::EPERM as u32),
        instruction(0x06, 0, 0, 0x7fff0000),
    ]);
    let program = libc::sock_fprog {
        len: code.len() as u16,
        filter: code.as_mut_ptr(),
    };
    // SAFETY: the kernel copies a live BPF instruction array synchronously.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::prctl(libc::PR_SET_SECCOMP, 2, &program) } != 0
    {
        return Err(format!(
            "plugin process restrictions unavailable: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

// dyld requires reading the root directory itself during bootstrap. This is a
// literal grant, not recursive access to user files or arbitrary Mach services.
#[cfg(any(test, target_os = "macos"))]
pub(super) fn macos_profile(policy: &Policy) -> String {
    let quote = |path: &Path| {
        format!(
            "\"{}\"",
            path.to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
        )
    };
    let mut profile = String::from("(version 1)(deny default)(allow process-info* (target self))(allow sysctl-read)(allow signal (target self))(allow file-read* (literal \"/\"))");
    profile.push_str(&format!(
        "(allow process-exec (literal {}))",
        quote(&policy.executable)
    ));
    for path in &policy.read_roots {
        profile.push_str(&format!("(allow file-read* (subpath {}))", quote(path)));
    }
    // The only writable path is the private scratch directory created by
    // `spawn`; user directories and the install package remain read-only.
    for path in &policy.write_roots {
        profile.push_str(&format!(
            "(allow file-read* file-write* (subpath {}))",
            quote(path)
        ));
        for ancestor in path.ancestors().skip(1) {
            profile.push_str(&format!(
                "(allow file-read-metadata (literal {}))",
                quote(ancestor)
            ));
        }
    }
    // Resolve only the allowed paths' ancestors; never grant blanket metadata.
    for path in &policy.read_roots {
        for ancestor in path.ancestors().skip(1) {
            profile.push_str(&format!(
                "(allow file-read-metadata (literal {}))",
                quote(ancestor)
            ));
        }
    }
    for ancestor in [
        "/System",
        "/Library",
        "/Library/Apple",
        "/Library/Apple/System",
    ] {
        profile.push_str(&format!(
            "(allow file-read-metadata (literal {}))",
            quote(Path::new(ancestor))
        ));
    }
    // Runtime system resources only; never permit sockets, forks or user paths.
    profile.push_str("(allow file-read* (subpath \"/System/Library\") (subpath \"/System/Cryptexes\") (subpath \"/Library/Apple/System/Library\") (literal \"/dev/null\") (literal \"/dev/urandom\"))");
    profile
}

/// The environment a plugin receives. It is deliberately small: no credentials
/// or user configuration, and every temporary directory points at the private
/// scratch space.
#[cfg(target_os = "linux")]
pub(super) fn plugin_env(scratch: &Path) -> Vec<(String, String)> {
    let scratch = scratch.to_string_lossy().into_owned();
    let mut env = vec![
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into()),
        ),
        (
            "LANG".to_string(),
            std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".into()),
        ),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
        ("TMPDIR".to_string(), scratch.clone()),
        ("TMP".to_string(), scratch.clone()),
        ("TEMP".to_string(), scratch),
    ];
    if let Some(home) = dirs::home_dir() {
        env.push(("HOME".to_string(), home.to_string_lossy().into_owned()));
    }
    env
}

#[cfg(target_os = "macos")]
pub(super) fn plugin_env(scratch: &Path) -> Vec<(String, String)> {
    let scratch = scratch.to_string_lossy().into_owned();
    vec![
        ("LANG".to_string(), "en_US.UTF-8".to_string()),
        ("TMPDIR".to_string(), scratch.clone()),
        ("TMP".to_string(), scratch.clone()),
        ("TEMP".to_string(), scratch),
    ]
}

#[cfg(windows)]
pub(super) fn plugin_env(scratch: &str) -> Vec<(String, String)> {
    let mut env = vec![
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_default(),
        ),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
    ];
    if !scratch.is_empty() {
        for key in ["TMPDIR", "TMP", "TEMP"] {
            env.push((key.to_string(), scratch.to_string()));
        }
    }
    if let Some(home) = dirs::home_dir() {
        env.push(("HOME".to_string(), home.to_string_lossy().into_owned()));
    }
    env
}

#[cfg(all(test, windows))]
mod windows_tests {
    #[test]
    fn sandbox_system_roots_exist() {
        for root in super::system_read_roots() {
            assert!(root.exists(), "missing sandbox root: {}", root.display());
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::{macos_profile, Policy};

    #[test]
    fn macos_profile_only_grants_runtime_paths_and_initial_executable() {
        let grant = crate::plugin_permissions::PluginGrant::default();
        let policy = Policy::new(
            std::path::Path::new("/plugin/bin"),
            std::path::Path::new("/plugin"),
            &grant,
            &Default::default(),
        );
        let profile = macos_profile(&policy);
        assert!(profile.contains("(allow file-read* (literal \"/\"))"));
        assert!(!profile.contains("(allow file-read-metadata)"));
        assert!(!profile.contains("(allow process-fork"));
        assert!(!profile.contains("(allow network"));
        assert!(!profile.contains("(allow mach-lookup"));
        assert!(!profile.contains("(allow process-exec (subpath"));
    }

    #[test]
    fn user_grants_never_open_native_files_or_network() {
        let grant = crate::plugin_permissions::PluginGrant {
            granted: ["network".into(), "files.write".into()].into(),
            ..Default::default()
        };
        let policy = Policy::new(
            std::path::Path::new("/plugin/bin"),
            std::path::Path::new("/plugin"),
            &grant,
            &grant.granted,
        );
        assert!(!policy.network);
        assert!(policy.write_roots.is_empty());
        if let Some(home) = dirs::home_dir() {
            assert!(!policy.read_roots.contains(&home));
        }
    }
}
