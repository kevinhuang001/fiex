#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn only_native_driver_absence_enables_usb_provider_fallback() {
    assert!(super::fallback::native_mount_unsupported(
        "unknown filesystem type 'btrfs'"
    ));
    assert!(!super::fallback::native_mount_unsupported(
        "Permission denied"
    ));
    assert!(!super::fallback::native_mount_unsupported(
        "Operation not permitted"
    ));
    assert!(!super::fallback::native_mount_unsupported(
        "wrong fs type, bad option, bad superblock"
    ));
    assert!(!super::fallback::native_mount_unsupported(
        "unknown filesystem type 'btrfs'\nError executing command as another user: Not authorized"
    ));
    assert!(!super::fallback::native_mount_unsupported(
        "unknown filesystem type 'btrfs'\nAuthentication is required to mount the device"
    ));
}

#[cfg(windows)]
#[test]
fn windows_access_denied_never_enables_raw_provider_fallback() {
    assert!(super::fallback::windows_native_mount_unsupported(
        &std::io::Error::from_raw_os_error(1005)
    ));
    assert!(!super::fallback::windows_native_mount_unsupported(
        &std::io::Error::from_raw_os_error(5)
    ));
}

#[cfg(feature = "usbfs")]
#[test]
fn only_btrfs_and_apple_filesystems_are_provider_fallbacks() {
    for filesystem in ["btrfs", "apfs", "hfs+"] {
        assert!(
            super::fallback::fallback_mount(
                "test-device",
                "/dev/nonexistent-fallback-test",
                "/dev/nonexistent-fallback-test",
                filesystem,
            )
            .is_some()
        );
    }
    assert!(
        super::fallback::fallback_mount(
            "test-device",
            "/dev/nonexistent-fallback-test",
            "/dev/nonexistent-fallback-test",
            "ntfs",
        )
        .is_none()
    );
    assert!(
        super::fallback::fallback_mount(
            "test-device",
            "/dev/nonexistent-fallback-test",
            "/dev/nonexistent-fallback-test",
            "ext4",
        )
        .is_none()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_hides_synthetic_apfs_volumes() {
    assert!(super::platform::visible_mount("/"));
    assert!(super::platform::visible_mount("/Volumes/External"));
    assert!(!super::platform::visible_mount("/System/Volumes/Data"));
    assert!(!super::platform::visible_mount("/System/Volumes/VM"));
}
