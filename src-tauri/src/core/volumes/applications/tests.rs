use super::*;

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn desktop_entries_require_an_application_with_a_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("editor.desktop");
        std::fs::write(
            &file,
            "[Desktop Entry]\nType=Application\nName=Editor\nNoDisplay=false\n",
        )
        .unwrap();
        assert_eq!(
            platform::desktop_entry(&file),
            Some(("Editor".into(), false, true))
        );
        std::fs::write(
            &file,
            "[Desktop Entry]\nType=Application\nName=Hidden\nNoDisplay=true\n",
        )
        .unwrap();
        assert_eq!(
            platform::desktop_entry(&file),
            Some(("Hidden".into(), true, true))
        );
        std::fs::write(&file, "[Desktop Entry]\nType=Link\nName=Link\n").unwrap();
        assert_eq!(
            platform::desktop_entry(&file),
            Some(("Link".into(), false, false))
        );
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_target_tests {
    use super::platform::resolve_windows_executable;

    #[test]
    fn valid_system_known_folder_target_is_launchable() {
        let target = resolve_windows_executable(r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\cmd.exe")
            .expect("Windows System32 cmd.exe should exist");
        assert!(target.to_lowercase().ends_with(r"\system32\cmd.exe"));
    }

    #[test]
    fn unresolved_and_missing_targets_are_not_listed() {
        assert!(
            resolve_windows_executable(r"{00000000-0000-0000-0000-000000000000}\missing.exe")
                .is_none()
        );
        assert!(resolve_windows_executable(
            r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\fiex-nonexistent-test.exe"
        )
        .is_none());
        assert!(resolve_windows_executable(r"C:\fiex-nonexistent-test.exe").is_none());
        assert!(resolve_windows_executable(r"C:\Windows\readme.txt").is_none());
    }
}

#[cfg(test)]
mod executable_target_tests {
    use super::is_executable_target;

    #[test]
    fn executables_are_kept_including_uninstallers() {
        for path in [
            r"C:\Program Files\App\app.exe",
            r"C:\Program Files\App\unins000.exe",
            r"C:\Program Files\App\Uninstall App.exe",
            r"C:\Program Files\App\tool.cmd",
            r"C:\Windows\System32\app.msc",
            r"C:\Users\me\AppData\Roaming\Microsoft\Installer\{ABC}\app.appref-ms",
        ] {
            assert!(is_executable_target(path), "{path} should be launched");
        }
    }

    #[test]
    fn documents_and_websites_are_filtered() {
        for path in [
            r"C:\Program Files\App\readme.pdf",
            r"C:\Program Files\App\manual.docx",
            r"C:\Program Files\App\release-notes.txt",
            "https://example.com",
            "https://example.com/download",
            r"C:\Program Files\App\website.url",
            "",
        ] {
            assert!(!is_executable_target(path), "{path} should be filtered");
        }
    }
}

#[cfg(test)]
mod folder_listing_tests {
    use super::*;

    #[test]
    fn system_application_metadata_is_read_only_even_before_capability_matching() {
        let protected = entry(
            "System App".into(),
            "applications://c3lzdGVt".into(),
            "app",
            0,
            true,
        );
        assert!(protected.readonly);
        assert_eq!(protected.allowed_ops, vec!["stat"]);
        let user = entry(
            "Shortcut".into(),
            "applications://~dXNlcg".into(),
            "app",
            0,
            false,
        );
        assert!(!user.readonly);
        assert!(user.allowed_ops.iter().any(|op| op == "remove"));
    }

    #[test]
    fn user_uris_use_the_canonical_store_prefix() {
        assert_eq!(user_encoded("applications://~abc"), Some("abc"));
        assert!(is_user("applications://~abc"));
        assert!(!is_user("applications://user/abc"));
        // System application identities are never user-store nodes.
        assert!(!is_user("applications://YWJj"));
    }

    fn item(name: &str, path: &str, is_dir: bool) -> Entry {
        Entry {
            name: name.into(),
            path: path.into(),
            is_dir,
            ..Default::default()
        }
    }

    #[test]
    fn mapped_system_apps_leave_the_root_and_appear_once_in_the_folder() {
        let app = item("Editor", "applications://YXBw", false);
        let other = item("Browser", "applications://YnJvd3Nlcg", false);
        let folder = item("Work", "applications://~d29yaw", true);
        let mapped = HashSet::from([app.path.clone()]);
        let root = root_listing(
            vec![app.clone(), other.clone()],
            vec![folder.clone()],
            &mapped,
        );
        assert!(root.iter().any(|entry| entry.path == other.path));
        assert!(root.iter().any(|entry| entry.path == folder.path));
        assert!(!root.iter().any(|entry| entry.path == app.path));

        let children = folder_listing(
            vec![folder],
            std::slice::from_ref(&app),
            std::slice::from_ref(&app.path),
        );
        assert_eq!(
            children
                .iter()
                .filter(|entry| entry.path == app.path)
                .count(),
            1
        );
    }

    #[test]
    fn duplicate_real_and_mapped_entries_are_collapsed() {
        let app = item("Editor", "applications://YXBw", false);
        let entries = folder_listing(
            vec![app.clone()],
            std::slice::from_ref(&app),
            std::slice::from_ref(&app.path),
        );
        assert_eq!(entries.len(), 1);
    }
}
