//! Opaque Trash identities on every platform. Backing paths never leave Core.
use super::recycle;
#[cfg(not(target_os = "windows"))]
use super::trash;
#[cfg(not(target_os = "windows"))]
use crate::core::files;
use crate::core::files::Entry;
#[cfg(not(target_os = "windows"))]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
#[cfg(not(target_os = "windows"))]
use std::path::{Component, Path, PathBuf};
pub const URI: &str = "trash://";
pub fn handles(path: &str) -> bool {
    path.starts_with(URI)
}
#[cfg(not(target_os = "windows"))]
fn failure(error: std::io::Error) -> String {
    #[cfg(target_os = "macos")]
    if error.kind() == std::io::ErrorKind::PermissionDenied || error.raw_os_error() == Some(1) {
        return format!("PermissionDenied: {}", crate::i18n::t("Fiex 无权访问废纸篓。请打开“系统设置 → 隐私与安全性 → 完全磁盘访问权限”，添加并开启 Fiex，然后退出并重新打开 Fiex，再访问废纸篓。"));
    }
    error.to_string()
}
/// Check access before painting cached metadata; revocation must not revive old entries.
pub fn check_access() -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let root = trash::trash_path().ok_or(crate::i18n::t("无法定位废纸篓"))?;
        match std::fs::read_dir(root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(failure(error)),
        }
    }
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn relative(uri: &str) -> Result<PathBuf, String> {
    let encoded = uri
        .strip_prefix(URI)
        .ok_or("InvalidVfsUri: invalid Trash identity")?;
    if encoded.is_empty() {
        return Ok(PathBuf::new());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "InvalidVfsUri: invalid Trash node")?;
    let text = String::from_utf8(bytes).map_err(|_| "InvalidVfsUri: invalid Trash node")?;
    let path = PathBuf::from(text);
    if path.as_os_str().is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("ScopeDenied: Trash node escapes its root".into());
    }
    Ok(path)
}
#[cfg(not(target_os = "windows"))]
pub fn physical(uri: &str) -> Result<PathBuf, String> {
    let root = trash::trash_path().ok_or(crate::i18n::t("无法定位废纸篓"))?;
    confined(&root, &relative(uri)?)
}
#[cfg(not(target_os = "windows"))]
fn confined(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let target = root.join(relative);
    let mut current = root.to_path_buf();
    let components: Vec<_> = relative.components().collect();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        current.push(component.as_os_str());
        if std::fs::symlink_metadata(&current)
            .map_err(failure)?
            .file_type()
            .is_symlink()
        {
            return Err("ScopeDenied: cannot traverse a Trash symlink".into());
        }
    }
    Ok(target)
}
pub fn action_path(path: &str) -> Result<String, String> {
    if !handles(path) {
        return Ok(path.into());
    }
    if path == URI {
        return Err("ScopeDenied: cannot mutate the Trash root".into());
    }
    #[cfg(not(target_os = "windows"))]
    {
        physical(path).map(|p| p.to_string_lossy().into_owned())
    }
    #[cfg(target_os = "windows")]
    {
        Err("InvalidVfsUri: use the issued Recycle Bin node identity".into())
    }
}
pub fn list(uri: &str) -> Result<Vec<Entry>, String> {
    check_access()?;
    #[cfg(target_os = "windows")]
    {
        let _ = uri;
        recycle::list()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let root = trash::trash_path().ok_or(crate::i18n::t("无法定位废纸篓"))?;
        let path = physical(uri)?;
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("ScopeDenied: cannot browse a Trash symlink".into());
        }
        if !path.exists() {
            return Ok(Vec::new());
        }
        let listing = files::list(&path.to_string_lossy(), true, &[])?;
        listing
            .entries
            .into_iter()
            .map(|mut entry| {
                let relative = Path::new(&entry.path)
                    .strip_prefix(&root)
                    .map_err(|_| "ScopeDenied: Trash listing escaped its root")?;
                entry.path = format!(
                    "{URI}{}",
                    URL_SAFE_NO_PAD.encode(relative.to_string_lossy().as_bytes())
                );
                entry.link_target = None;
                entry.allowed_ops = if entry.is_dir && !entry.is_symlink {
                    vec!["stat".into(), "list".into()]
                } else {
                    vec!["stat".into()]
                };
                Ok(entry)
            })
            .collect()
    }
}
pub fn entry(uri: &str) -> Result<Entry, String> {
    if uri == URI {
        return Ok(Entry {
            path: URI.into(),
            name: crate::i18n::t("废纸篓").into(),
            is_dir: true,
            allowed_ops: vec!["list".into(), "stat".into()],
            ..Default::default()
        });
    }
    #[cfg(target_os = "windows")]
    {
        recycle::list()?
            .into_iter()
            .find(|e| e.path == uri)
            .ok_or_else(|| "NotFound: Trash node missing".into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut entry = files::entry(&physical(uri)?)?;
        entry.path = uri.into();
        entry.link_target = None;
        Ok(entry)
    }
}
pub fn is_tree(path: &str) -> bool {
    handles(path) || path == recycle::URI
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn identities_cannot_escape_the_trash() {
        for value in ["../outside", "/etc/passwd", "a/../../outside"] {
            let uri = format!("{URI}{}", URL_SAFE_NO_PAD.encode(value));
            assert!(relative(&uri).is_err());
        }
        assert!(action_path(URI).is_err());
        assert_eq!(
            relative(&format!("{URI}{}", URL_SAFE_NO_PAD.encode("folder/file"))).unwrap(),
            PathBuf::from("folder/file")
        );
    }
    #[cfg(unix)]
    #[test]
    fn nested_symlink_is_never_followed() {
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/tmp", root.path().join("escape")).unwrap();
        assert!(confined(root.path(), Path::new("escape/child")).is_err());
    }
}
