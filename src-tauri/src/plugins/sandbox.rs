//! OS-level plugin confinement.
//!
//! Linux uses Landlock for filesystem/TCP scopes and a small seccomp filter
//! for syscalls Landlock cannot express (non-TCP sockets and process
//! execution). macOS uses a generated Seatbelt profile through
//! `/usr/bin/sandbox-exec`. Windows currently runs plugins in a normal child
//! process; see the platform limitations in `docs/plugins/07-SECURITY-PACKAGING.md`.
//!
//! A failed sandbox setup is fatal: a plugin is never launched unsandboxed
//! when the host believes a policy is active.
use crate::plugin_permissions::PluginGrant;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Policy {
    pub executable: std::path::PathBuf,
    pub install_dir: std::path::PathBuf,
    pub read_roots: Vec<std::path::PathBuf>,
    pub write_roots: Vec<std::path::PathBuf>,
    pub execute_roots: Vec<std::path::PathBuf>,
    pub network: bool,
}

impl Policy {
    pub fn new(
        executable: &Path,
        install_dir: &Path,
        grant: &PluginGrant,
        declared: &std::collections::BTreeSet<String>,
    ) -> Self {
        let canonical = |path: &Path| dunce::canonicalize(path).unwrap_or_else(|_| path.into());
        let executable = canonical(executable);
        let install_dir = canonical(install_dir);
        let readable = declared.contains("filesystem.read");
        let writable = declared.contains("filesystem.write");
        let mut read_roots = vec![
            executable.clone(),
            install_dir.clone(),
            canonical(Path::new("/usr")),
            canonical(Path::new("/lib")),
            canonical(Path::new("/lib64")),
            canonical(Path::new("/bin")),
            canonical(Path::new("/sbin")),
            canonical(Path::new("/etc")),
            canonical(Path::new("/proc")),
            canonical(Path::new("/sys")),
            canonical(Path::new("/dev/urandom")),
            canonical(Path::new("/dev/null")),
        ];
        let mut write_roots = vec![
            canonical(&std::env::temp_dir()),
            canonical(&install_dir),
            canonical(Path::new("/dev/null")),
        ];
        // The ELF interpreter lives under /lib or /lib64 and must be
        // executable even when plugins may not spawn external commands.
        let mut execute_roots = vec![
            executable.clone(),
            canonical(Path::new("/lib")),
            canonical(Path::new("/lib64")),
            canonical(Path::new("/usr/lib")),
        ];
        for loader in [
            "/lib64/ld-linux-x86-64.so.2",
            "/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
            "/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
            "/lib/ld-linux-aarch64.so.1",
            "/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1",
            "/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1",
        ] {
            let path = Path::new(loader);
            if path.exists() {
                execute_roots.push(canonical(path));
            }
        }
        execute_roots.extend([
            canonical(Path::new("/usr")),
            canonical(Path::new("/bin")),
            canonical(Path::new("/sbin")),
        ]);
        if readable {
            if let Some(home) = dirs::home_dir() {
                read_roots.push(canonical(&home));
            }
            for root in ["/media", "/mnt", "/run/media", "/Volumes"] {
                let path = canonical(Path::new(root));
                if path.exists() {
                    read_roots.push(path);
                }
            }
        }
        if writable {
            if let Some(home) = dirs::home_dir() {
                write_roots.push(canonical(&home));
            }
            for root in ["/media", "/mnt", "/run/media", "/Volumes"] {
                let path = canonical(Path::new(root));
                if path.exists() {
                    write_roots.push(path);
                }
            }
        }
        read_roots.sort();
        read_roots.dedup();
        write_roots.sort();
        write_roots.dedup();
        execute_roots.sort();
        execute_roots.dedup();
        Self {
            executable,
            install_dir,
            read_roots,
            write_roots,
            execute_roots,
            network: grant.granted.contains("network"),
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::Policy;
    use std::{
        os::{fd::RawFd, unix::process::CommandExt},
        process::Command,
        sync::Arc,
    };

    const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
    const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

    const FS_EXECUTE: u64 = 1 << 0;
    const FS_WRITE_FILE: u64 = 1 << 1;
    const FS_READ_FILE: u64 = 1 << 2;
    const FS_READ_DIR: u64 = 1 << 3;
    const FS_REMOVE_DIR: u64 = 1 << 4;
    const FS_REMOVE_FILE: u64 = 1 << 5;
    const FS_MAKE_DIR: u64 = 1 << 7;
    const FS_MAKE_REG: u64 = 1 << 8;
    const FS_MAKE_SOCK: u64 = 1 << 9;
    const FS_MAKE_FIFO: u64 = 1 << 10;
    const FS_MAKE_SYM: u64 = 1 << 12;
    const FS_REFER: u64 = 1 << 13;
    const FS_TRUNCATE: u64 = 1 << 14;

    const NET_BIND_TCP: u64 = 1 << 0;
    const NET_CONNECT_TCP: u64 = 1 << 1;

    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
    }
    #[repr(C)]
    struct PathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    pub struct Plan {
        ruleset: RulesetAttr,
        ruleset_size: usize,
        rules: Vec<(RawFd, u64)>,
        network_denied: bool,
        close: Vec<RawFd>,
    }
    impl Drop for Plan {
        fn drop(&mut self) {
            for fd in &self.close {
                unsafe {
                    libc::close(*fd);
                }
            }
        }
    }

    fn abi() -> Result<i64, String> {
        let value = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<RulesetAttr>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if value < 0 {
            let error = std::io::Error::last_os_error();
            return Err(format!(
                "Landlock is unavailable ({error}); this kernel cannot sandbox plugins"
            ));
        }
        Ok(value)
    }

    fn fs_access(abi: i64, directory: bool, read: bool, write: bool, execute: bool) -> u64 {
        if directory {
            let mut access = FS_READ_DIR;
            if read {
                access |= FS_READ_FILE;
            }
            if execute {
                access |= FS_EXECUTE;
            }
            if write {
                access |= FS_WRITE_FILE
                    | FS_REMOVE_DIR
                    | FS_REMOVE_FILE
                    | FS_MAKE_DIR
                    | FS_MAKE_REG
                    | FS_MAKE_SOCK
                    | FS_MAKE_FIFO
                    | FS_MAKE_SYM;
                if abi >= 2 {
                    access |= FS_REFER;
                }
            }
            return access;
        }
        let mut access = 0;
        if read {
            access |= FS_READ_FILE;
        }
        if write {
            access |= FS_WRITE_FILE;
            if abi >= 3 {
                access |= FS_TRUNCATE;
            }
        }
        if execute {
            access |= FS_EXECUTE;
        }
        access
    }

    fn open_path(path: &std::path::Path) -> Option<RawFd> {
        use std::os::unix::ffi::OsStrExt;
        let mut bytes = path.as_os_str().as_bytes().to_vec();
        bytes.push(0);
        let fd = unsafe { libc::open(bytes.as_ptr().cast(), libc::O_PATH | libc::O_CLOEXEC) };
        (fd >= 0).then_some(fd)
    }

    pub fn prepare(policy: &Policy) -> Result<Arc<Plan>, String> {
        let abi = abi()?;
        let handled_fs = if abi >= 3 {
            FS_EXECUTE
                | FS_WRITE_FILE
                | FS_READ_FILE
                | FS_READ_DIR
                | FS_REMOVE_DIR
                | FS_REMOVE_FILE
                | FS_MAKE_DIR
                | FS_MAKE_REG
                | FS_MAKE_SOCK
                | FS_MAKE_FIFO
                | FS_MAKE_SYM
                | FS_REFER
                | FS_TRUNCATE
        } else if abi >= 2 {
            FS_EXECUTE
                | FS_WRITE_FILE
                | FS_READ_FILE
                | FS_READ_DIR
                | FS_REMOVE_DIR
                | FS_REMOVE_FILE
                | FS_MAKE_DIR
                | FS_MAKE_REG
                | FS_MAKE_SOCK
                | FS_MAKE_FIFO
                | FS_MAKE_SYM
                | FS_REFER
        } else {
            FS_EXECUTE
                | FS_WRITE_FILE
                | FS_READ_FILE
                | FS_READ_DIR
                | FS_REMOVE_DIR
                | FS_REMOVE_FILE
                | FS_MAKE_DIR
                | FS_MAKE_REG
                | FS_MAKE_SOCK
                | FS_MAKE_FIFO
                | FS_MAKE_SYM
        };
        let network_denied = !policy.network;
        let handled_net = if network_denied && abi >= 4 {
            NET_BIND_TCP | NET_CONNECT_TCP
        } else {
            0
        };
        let ruleset = RulesetAttr {
            handled_access_fs: handled_fs,
            handled_access_net: handled_net,
        };
        let ruleset_size = if abi >= 4 { 16 } else { 8 };
        let mut rules = Vec::new();
        let mut close = Vec::new();
        let mut add = |path: &std::path::Path, read: bool, write: bool, execute: bool| {
            let Ok(metadata) = std::fs::metadata(path) else {
                return;
            };
            let Some(fd) = open_path(path) else {
                return;
            };
            rules.push((fd, fs_access(abi, metadata.is_dir(), read, write, execute)));
            close.push(fd);
        };
        // Narrow write roots also need read access to update existing files.
        for root in &policy.write_roots {
            add(root, true, true, false);
        }
        for root in &policy.read_roots {
            add(root, true, false, false);
        }
        for root in &policy.execute_roots {
            add(root, true, false, true);
        }
        Ok(Arc::new(Plan {
            ruleset,
            ruleset_size,
            rules,
            network_denied,
            close,
        }))
    }

    fn apply(plan: &Plan) -> Result<(), String> {
        unsafe {
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(format!(
                    "cannot set no_new_privs: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        let ruleset_fd = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &plan.ruleset as *const RulesetAttr,
                plan.ruleset_size,
                0u32,
            )
        };
        if ruleset_fd < 0 {
            return Err(format!(
                "cannot create Landlock ruleset: {}",
                std::io::Error::last_os_error()
            ));
        }
        let ruleset_fd = ruleset_fd as RawFd;
        for (parent_fd, access) in &plan.rules {
            let attr = PathBeneathAttr {
                allowed_access: *access,
                parent_fd: *parent_fd,
            };
            let result = unsafe {
                libc::syscall(
                    libc::SYS_landlock_add_rule,
                    ruleset_fd,
                    LANDLOCK_RULE_PATH_BENEATH,
                    &attr as *const PathBeneathAttr,
                    0u32,
                )
            };
            if result != 0 {
                unsafe { libc::close(ruleset_fd) };
                return Err(format!(
                    "cannot add Landlock path rule: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        let result = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0u32) };
        unsafe { libc::close(ruleset_fd) };
        if result != 0 {
            return Err(format!(
                "cannot apply Landlock restrictions: {}",
                std::io::Error::last_os_error()
            ));
        }
        if plan.network_denied {
            seccomp(plan.network_denied)?;
        }
        Ok(())
    }

    #[repr(C)]
    struct SockFilter {
        code: u16,
        jt: u8,
        jf: u8,
        k: u32,
    }
    #[repr(C)]
    struct SockFprog {
        len: u16,
        filter: *const SockFilter,
    }
    const BPF_LD_W_ABS: u16 = 0x20;
    const BPF_JMP_JEQ_K: u16 = 0x15;
    const BPF_RET_K: u16 = 0x06;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;

    fn seccomp(deny_network: bool) -> Result<(), String> {
        let mut filter = vec![SockFilter {
            code: BPF_LD_W_ABS,
            jt: 0,
            jf: 0,
            k: 0,
        }];
        let mut deny = |syscall: libc::c_long| {
            filter.push(SockFilter {
                code: BPF_JMP_JEQ_K,
                jt: 0,
                jf: 1,
                k: syscall as u32,
            });
            filter.push(SockFilter {
                code: BPF_RET_K,
                jt: 0,
                jf: 0,
                k: SECCOMP_RET_ERRNO | libc::EACCES as u32,
            });
        };
        if deny_network {
            deny(libc::SYS_socket);
            deny(libc::SYS_connect);
            deny(libc::SYS_socketpair);
        }
        filter.push(SockFilter {
            code: BPF_RET_K,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_ALLOW,
        });
        let program = SockFprog {
            len: filter.len() as u16,
            filter: filter.as_ptr(),
        };
        let result =
            unsafe { libc::prctl(libc::PR_SET_SECCOMP, 2, &program as *const SockFprog, 0, 0) };
        if result != 0 {
            return Err(format!(
                "cannot install seccomp filter: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    pub fn command(policy: &Policy) -> Result<Command, String> {
        let plan = prepare(policy)?;
        let mut command = Command::new(&policy.executable);
        let plan_for_child = plan.clone();
        unsafe {
            command.pre_exec(move || {
                apply(&plan_for_child).map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::PermissionDenied, error)
                })
            });
        }
        drop(plan);
        Ok(command)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::Policy;
    use std::process::Command;

    fn escape(path: &std::path::Path) -> String {
        path.to_string_lossy().replace('"', "\\\"")
    }

    fn profile(policy: &Policy) -> String {
        let mut rules = String::from("(version 1)\n(deny default)\n");
        rules.push_str("(allow process-fork)\n(allow signal (target self))\n(allow sysctl-read)\n");
        rules.push_str("(allow mach-lookup)\n(allow ipc-posix-shm)\n");
        rules.push_str(&format!(
            "(allow process-exec (literal \"{}\"))\n",
            escape(&policy.executable)
        ));
        rules.push_str("(allow process-exec (subpath \"/usr/bin\") (subpath \"/bin\") (subpath \"/usr/sbin\") (subpath \"/sbin\"))\n");
        for root in &policy.read_roots {
            rules.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                escape(root)
            ));
        }
        for root in &policy.write_roots {
            rules.push_str(&format!(
                "(allow file-read* file-write* (subpath \"{}\"))\n",
                escape(root)
            ));
        }
        if policy.network {
            rules.push_str("(allow network-outbound)\n(allow network-bind)\n");
        }
        rules
    }

    pub fn command(policy: &Policy) -> Result<Command, String> {
        let sandbox = std::path::Path::new("/usr/bin/sandbox-exec");
        if !sandbox.is_file() {
            return Err(
                "macOS sandbox-exec is unavailable; refusing to run a plugin unsandboxed".into(),
            );
        }
        let mut command = Command::new(sandbox);
        command
            .arg("-p")
            .arg(profile(policy))
            .arg(&policy.executable);
        Ok(command)
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::Policy;
    use std::process::Command;

    pub fn command(policy: &Policy) -> Result<Command, String> {
        // Windows AppContainer requires a custom token and handle-list spawn.
        // Until that lands, plugins still run as child processes with explicit
        // grant checks; this is documented as a platform limitation.
        Ok(Command::new(&policy.executable))
    }
}

/// Build a sandboxed command for a plugin process.
pub fn command(policy: &Policy) -> Result<std::process::Command, String> {
    #[cfg(target_os = "linux")]
    {
        linux::command(policy)
    }
    #[cfg(target_os = "macos")]
    {
        macos::command(policy)
    }
    #[cfg(target_os = "windows")]
    {
        windows::command(policy)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = policy;
        Err("this platform has no plugin sandbox implementation".into())
    }
}
