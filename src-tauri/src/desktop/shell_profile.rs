//! Shell profile startup flags for the integrated terminal.

pub(crate) fn login_arg(shell: &str) -> Option<&'static str> {
    match std::path::Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
    {
        "zsh" | "bash" | "fish" | "sh" | "dash" | "ksh" | "tcsh" | "csh" => Some("-l"),
        "nu" => Some("--login"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::login_arg;

    #[test]
    fn profiles_are_loaded_for_supported_shells() {
        assert_eq!(login_arg("/bin/zsh"), Some("-l"));
        assert_eq!(login_arg("/opt/homebrew/bin/fish"), Some("-l"));
        assert_eq!(login_arg("/opt/homebrew/bin/nu"), Some("--login"));
        assert_eq!(login_arg("powershell.exe"), None);
    }
}
