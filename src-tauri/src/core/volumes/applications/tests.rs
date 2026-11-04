use super::*;

#[cfg(all(test, target_os = "linux"))]
mod linux_platform_tests {
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
