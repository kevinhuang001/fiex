use std::path::Path;
#[cfg(any(not(windows), test))]
use std::path::PathBuf;
/// Shells that read a user profile and provide an editable prompt. `sh` and
/// `dash` only qualify as a last resort; multiplexers listed in `/etc/shells`
/// (`tmux`, `screen`) are not shells at all.
#[cfg(any(not(windows), test))]
pub(crate) const PROFILE_SHELLS: [&str; 11] = [
    "bash", "zsh", "fish", "ksh", "tcsh", "csh", "nu", "elvish", "mksh", "ash", "xonsh",
];
#[cfg(any(not(windows), test))]
pub(crate) const POSIX_SHELLS: [&str; 2] = ["sh", "dash"];

pub(crate) fn shell_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(any(not(windows), test))]
pub(crate) fn shell_rank(path: &str) -> u8 {
    let name = shell_name(path);
    if PROFILE_SHELLS.contains(&name.as_str()) {
        1
    } else if POSIX_SHELLS.contains(&name.as_str()) {
        2
    } else {
        3
    }
}

/// The user's own login shell leads, profile shells follow, POSIX `sh` comes
/// last so a fresh terminal already has the expected prompt and aliases.
#[cfg(any(not(windows), test))]
pub(crate) fn order_shells(login: Option<&str>, listed: Vec<String>) -> Vec<String> {
    let login = login
        .filter(|path| Path::new(path).is_file())
        .map(str::to_string);
    let mut candidates: Vec<String> = login.iter().cloned().collect();
    candidates.extend(listed);
    let mut ranked: Vec<(u8, usize, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in candidates {
        if path.ends_with(".exe") || !Path::new(&path).is_file() {
            continue;
        }
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path));
        if !seen.insert(canonical) {
            continue;
        }
        // The user's own login shell always leads, even when it is POSIX `sh`;
        // the rest follow shell quality so a fresh tab gets a profile prompt.
        let rank = if login.as_deref() == Some(path.as_str()) {
            0
        } else {
            shell_rank(&path)
        };
        ranked.push((rank, ranked.len(), path));
    }
    ranked.sort_by_key(|(rank, order, _)| (*rank, *order));
    // `/etc/shells` may list multiplexers such as tmux; only offer them when the
    // machine has no real shell at all.
    let has_real_shell = ranked.iter().any(|(rank, _, _)| *rank < 3);
    if has_real_shell {
        ranked.retain(|(rank, _, _)| *rank < 3);
    }
    ranked.into_iter().map(|(_, _, path)| path).collect()
}

#[cfg(test)]
mod shell_tests {
    use super::{order_shells, shell_rank, POSIX_SHELLS, PROFILE_SHELLS};

    #[test]
    fn profile_shells_outrank_posix_sh() {
        for name in PROFILE_SHELLS {
            let path = format!("/bin/{name}");
            assert_eq!(shell_rank(&path), 1, "{path}");
        }
        for name in POSIX_SHELLS {
            let path = format!("/usr/bin/{name}");
            assert_eq!(shell_rank(&path), 2, "{path}");
        }
        assert_eq!(shell_rank("/usr/bin/tmux"), 3);
        assert_eq!(shell_rank("powershell.exe"), 3);
    }

    // These ordering fixtures use real Unix shell paths.
    #[cfg(unix)]
    #[test]
    fn a_login_shell_leads_and_duplicates_collapse() {
        let ordered = order_shells(
            Some("/bin/sh"),
            vec![
                "/usr/bin/bash".into(),
                "/bin/bash".into(),
                "/usr/bin/tmux".into(),
            ],
        );
        assert_eq!(ordered.first().map(String::as_str), Some("/bin/sh"));
        assert_eq!(
            ordered.iter().filter(|path| path.ends_with("bash")).count(),
            1
        );
    }
}

/// Shell-specific startup options. Let each shell load its native profile/rc
/// (including ZDOTDIR, XDG_CONFIG_HOME and PowerShell's platform paths).
/// Never source a POSIX script into a different shell or suppress its profile.
pub(crate) fn startup_args(shell: &str) -> &'static [&'static str] {
    let name = shell
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(shell)
        .to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    match name {
        "bash" | "zsh" | "fish" | "sh" | "dash" | "ksh" | "mksh" | "ash" => &["-i", "-l"],
        // csh requires -l to be the only option; a PTY makes it interactive.
        "tcsh" | "csh" => &["-l"],
        "nu" => &["--login", "--interactive"],
        "pwsh" | "powershell" => &["-NoLogo"],
        // cmd loads AutoRun by default; elvish/xonsh load their rc on a PTY.
        "cmd" => &["/K"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::startup_args;
    #[test]
    fn flags_follow_the_shell_even_on_windows() {
        assert_eq!(startup_args("/bin/zsh"), &["-i", "-l"]);
        assert_eq!(
            startup_args("C:\\Program Files\\PowerShell\\7\\pwsh.exe"),
            &["-NoLogo"]
        );
        assert_eq!(startup_args("C:\\Windows\\System32\\cmd.exe"), &["/K"]);
        assert_eq!(startup_args("bash.exe"), &["-i", "-l"]);
        assert_eq!(startup_args("/usr/bin/nu"), &["--login", "--interactive"]);
        assert_eq!(startup_args("/bin/tcsh"), &["-l"]);
        assert_eq!(startup_args("elvish"), &[] as &[&str]);
    }
}
