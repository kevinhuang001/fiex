use super::*;

#[test]
fn reference_identity_is_scoped_to_mount_and_display() {
    let first = reference_for("applications://folder/app", "applications");
    assert_eq!(
        first,
        reference_for("applications://folder/app", "applications")
    );
    assert_ne!(
        first,
        reference_for("applications://folder/app", "another-mount")
    );
    assert_ne!(
        first,
        reference_for("applications://folder/other", "applications")
    );
}

#[test]
fn mime_mapping_keeps_resource_preview_types() {
    assert_eq!(mime_for("pdf"), "application/pdf");
    assert_eq!(mime_for("mkv"), "video/x-matroska");
    assert_eq!(mime_for("unknown-extension"), "application/octet-stream");
}

#[test]
fn capabilities_intersect_independent_operations() {
    let read_only = FileCapabilities {
        read: true,
        stat: true,
        ..FileCapabilities::default()
    };
    let writable = FileCapabilities {
        read: true,
        write: true,
        ..FileCapabilities::default()
    };
    assert!(read_only.intersect(writable).read);
    assert!(!read_only.intersect(writable).write);
}
