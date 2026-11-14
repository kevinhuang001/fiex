use crate::core::{
    fs::{ResourceBroker, provider_registry},
    operation_types::{Action, Resolution},
};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::AtomicBool;

fn replacement(source: &Path, target: &Path) -> Resolution {
    let (source_scope, source_ref) = super::binding(source.parent().unwrap(), source);
    let (target_scope, target_ref) = super::binding(target.parent().unwrap(), target);
    let source = ResourceBroker::resolve(&source_scope, "core:recovery-test", &source_ref).unwrap();
    let target = ResourceBroker::resolve(&target_scope, "core:recovery-test", &target_ref).unwrap();
    let source_version = source.stat().unwrap();
    let target_version = target.stat().unwrap();
    Resolution {
        key: format!(
            "{}\0{}\0{}\0{}\0{}\0{}",
            source.path,
            target.path,
            source_version["modified"].as_u64().unwrap_or(0),
            target_version["modified"].as_u64().unwrap_or(0),
            source_version["size"].as_u64().unwrap_or(0),
            target_version["size"].as_u64().unwrap_or(0),
        ),
        action: Action::Replace,
    }
}

struct ResetFaultInjection;

impl Drop for ResetFaultInjection {
    fn drop(&mut self) {
        provider_registry::clear_provider_write_failures_for_test();
        super::super::transfer::clear_recovery_base_for_test();
    }
}

#[test]
fn failed_provider_rollback_retains_overwritten_target_for_recovery() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let first_source = source.path().join("first.txt");
    let second_source = source.path().join("second.txt");
    let first_target = target.path().join("first.txt");
    let second_target = target.path().join("second.txt");
    std::fs::write(&first_source, b"replacement first payload").unwrap();
    std::fs::write(&second_source, b"replacement second payload").unwrap();
    std::fs::write(&first_target, b"original first").unwrap();
    std::fs::write(&second_target, b"original second").unwrap();

    let resolutions = [
        replacement(&first_source, &first_target),
        replacement(&second_source, &second_target),
    ];
    let _reset = ResetFaultInjection;
    provider_registry::fail_provider_writes_for_test(&[
        second_target.to_string_lossy().into_owned(),
        first_target.to_string_lossy().into_owned(),
    ]);
    let recovery_root = tempfile::tempdir().unwrap();
    super::super::transfer::set_recovery_base_for_test(recovery_root.path().to_path_buf());

    let files = [
        first_source.to_string_lossy().into_owned(),
        second_source.to_string_lossy().into_owned(),
    ];
    let result = crate::core::fs::transfer(
        &files,
        &target.path().to_string_lossy(),
        false,
        false,
        &[],
        &resolutions,
        &AtomicBool::new(false),
        &|_, _, _| {},
    )
    .unwrap();

    assert!(result.paths.is_empty());
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    let error = &result.errors[0];
    assert!(error.contains("injected provider write failure"), "{error}");
    assert!(error.contains("rollback incomplete"), "{error}");
    assert!(error.contains("recovery retained at"), "{error}");
    assert_eq!(
        std::fs::read(&first_target).unwrap(),
        b"replacement first payload"
    );
    assert_eq!(std::fs::read(&second_target).unwrap(), b"original second");
    assert_eq!(
        std::fs::read(first_source).unwrap(),
        b"replacement first payload"
    );
    assert_eq!(
        std::fs::read(second_source).unwrap(),
        b"replacement second payload"
    );

    let recovery_path = error
        .split_once("recovery retained at ")
        .map(|(_, path)| Path::new(path))
        .expect("recovery directory should be reported");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(recovery_path.join("recovery.json")).unwrap())
            .unwrap();
    let backup = manifest["backups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["target"] == first_target.to_string_lossy().as_ref())
        .and_then(|item| item["backup"].as_str())
        .expect("original overwritten target should have a recovery backup");
    assert_eq!(std::fs::read(backup).unwrap(), b"original first");
}
