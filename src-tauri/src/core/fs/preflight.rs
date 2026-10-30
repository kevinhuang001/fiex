//! Transfer review is additional policy, never a replacement for capabilities.
use super::*;
use crate::plugin_sdk::filesystem::{
    TransferAction, TransferDirection, TransferMetadata, TransferPreflight,
};

impl ResourceBroker {
    pub(crate) fn transfer_metadata(&self) -> Result<TransferMetadata, FsError> {
        let stat = self.stat()?;
        let entry = self.desktop_entry()?;
        Ok(TransferMetadata {
            name: entry.name,
            kind: if entry.is_dir {
                "directory"
            } else if entry.is_symlink {
                "symlink"
            } else {
                "file"
            }
            .into(),
            size: stat["size"].as_u64(),
            modified: stat["modified"].as_u64(),
            extension: Some(entry.extension),
            mime: Some(entry.mime),
            executable_shortcut: executable_shortcut(&self.path, entry.link_target.as_deref()),
            system: crate::core::volumes::applications::is_system_app(&self.path),
        })
    }
    pub(crate) fn review_transfer(
        &self,
        direction: TransferDirection,
        action: TransferAction,
        name: &str,
        metadata: &TransferMetadata,
        same_mount: bool,
    ) -> Result<(), FsError> {
        self.adapter()?.preflight(&TransferPreflight {
            node_id: self.reference.node_id.clone(),
            direction,
            action,
            name: name.into(),
            metadata: metadata.clone(),
            same_mount,
        })
    }
}
fn executable_shortcut(path: &str, target: Option<&str>) -> Option<bool> {
    // Provider paths carry no trusted host-path or executable-target authority.
    if path.contains("://") {
        return None;
    }
    #[cfg(target_os = "windows")]
    if std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
    {
        return Some(target.is_some_and(|target| {
            std::path::Path::new(target).is_file()
                && crate::core::volumes::applications::is_executable_target(target)
        }));
    }
    let _ = target;
    #[cfg(target_os = "linux")]
    if path.ends_with(".desktop") {
        let Ok(metadata) = std::fs::metadata(path) else {
            return Some(false);
        };
        if metadata.len() > 65536 {
            return Some(false);
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return Some(false);
        };
        let section = text.split("[Desktop Entry]").nth(1)?.split("\n[").next()?;
        let application = section
            .lines()
            .any(|line| line.trim() == "Type=Application");
        let command = section.lines().find_map(|line| line.strip_prefix("Exec="));
        return Some(
            application
                && command.is_some_and(|command| {
                    shell_words::split(command)
                        .ok()
                        .and_then(|words| words.into_iter().next())
                        .is_some_and(|first| which::which(first).is_ok())
                }),
        );
    }
    #[cfg(target_os = "macos")]
    {
        let app = std::path::Path::new(path);
        if app
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
        {
            return Some(
                app.is_dir()
                    && app.join("Contents/Info.plist").is_file()
                    && !std::fs::symlink_metadata(app).is_ok_and(|m| m.file_type().is_symlink()),
            );
        }
        // Finder aliases are regular files; resolving an alias never executes it.
        let kind = std::process::Command::new("mdls")
            .args(["-raw", "-name", "kMDItemContentType", path])
            .output()
            .ok()?;
        if String::from_utf8_lossy(&kind.stdout)
            .trim()
            .trim_matches('"')
            == "com.apple.alias-file"
        {
            let script = "on run argv\ntell application \"Finder\"\nset originalFile to original item of (POSIX file (item 1 of argv) as alias)\nreturn POSIX path of (originalFile as alias)\nend tell\nend run";
            let output = std::process::Command::new("osascript")
                .args(["-e", script, path])
                .output()
                .ok()?;
            if !output.status.success() {
                return Some(false);
            }
            let target = String::from_utf8(output.stdout).ok()?;
            let target = std::path::Path::new(target.trim());
            let metadata = std::fs::metadata(target).ok()?;
            use std::os::unix::fs::PermissionsExt;
            return Some(
                (metadata.is_dir()
                    && target
                        .extension()
                        .is_some_and(|extension| extension == "app"))
                    || (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0),
            );
        }
    }
    Some(crate::core::volumes::application_policy::executable_file(
        path,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn applications_rejects_document_import_before_any_write_or_source_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("preflight-document.txt");
        std::fs::write(&file, b"keep source").unwrap();
        let result = crate::core::fs::transfer(
            &[file.to_string_lossy().into_owned()],
            crate::core::volumes::applications::URI,
            true,
            false,
            &[],
            &[],
            &std::sync::atomic::AtomicBool::new(false),
            &|_, _, _| {},
        );
        assert!(result
            .err()
            .expect("transfer must be rejected")
            .contains("executable shortcuts"));
        assert_eq!(std::fs::read(&file).unwrap(), b"keep source");
    }
    #[test]
    fn metadata_unknown_is_not_a_proof_of_an_executable_shortcut() {
        assert_eq!(
            executable_shortcut("fiex://mount/example/shortcut.lnk", None),
            None
        );
        assert_eq!(executable_shortcut("/tmp/document.txt", None), Some(false));
    }
}
