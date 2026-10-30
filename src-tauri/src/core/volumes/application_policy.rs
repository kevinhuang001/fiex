//! Platform executable recognition and protection for installed macOS apps.
use std::path::Path;
pub fn protected(path: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        let path = dunce::canonicalize(path).unwrap_or_else(|_| Path::new(path).to_path_buf());
        [
            "/Applications",
            "/System/Applications",
            "/System/Library/CoreServices",
        ]
        .iter()
        .any(|root| path.starts_with(root))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}
pub fn executable_file(path: &str) -> bool {
    let path = Path::new(path);
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 != 0 {
            return true;
        }
    }
    #[cfg(target_os = "linux")]
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("appimage"))
    {
        use std::io::Read;
        let mut header = [0; 4];
        return std::fs::File::open(path)
            .and_then(|mut f| f.read_exact(&mut header))
            .is_ok()
            && header == *b"\x7fELF";
    }
    #[cfg(target_os = "windows")]
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        use std::io::Read;
        let mut header = [0; 2];
        return std::fs::File::open(path)
            .and_then(|mut f| f.read_exact(&mut header))
            .is_ok()
            && header == *b"MZ";
    }
    false
}
pub fn finish_import(uri: &str, executable: bool) -> Result<(), String> {
    #[cfg(unix)]
    if executable && super::applications::is_user(uri) {
        use std::os::unix::fs::PermissionsExt;
        let path = super::applications::candidate_path(uri)?;
        let mut permissions = std::fs::metadata(&path)
            .map_err(|e| e.to_string())?
            .permissions();
        permissions.set_mode((permissions.mode() & 0o777) | 0o100);
        std::fs::set_permissions(path, permissions).map_err(|e| e.to_string())?;
    }
    let _ = (uri, executable);
    Ok(())
}
