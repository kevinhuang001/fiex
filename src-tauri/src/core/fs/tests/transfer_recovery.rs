use crate::core::{
    fs::{ResourceBroker, provider_registry},
    operation_types::{Action, Resolution},
};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;

const RECOVERY_MANIFEST_ENV: &str = "FIEX_TEST_TRANSFER_RECOVERY_MANIFEST";
const RECOVERY_TARGET_ENV: &str = "FIEX_TEST_TRANSFER_RECOVERY_TARGET";
const RECOVERY_HELPER_ROOT_ENV: &str = "FIEX_TEST_TRANSFER_RECOVERY_HELPER_ROOT";

#[test]
fn preserved_recovery_manifest_is_readable_after_process_restart() {
    let Some(manifest_path) = std::env::var_os(RECOVERY_MANIFEST_ENV) else {
        return;
    };
    let target = dunce::canonicalize(
        std::env::var_os(RECOVERY_TARGET_ENV).expect("recovery target is required"),
    )
    .expect("recovery target exists");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(manifest_path).expect("recovery manifest exists"))
            .expect("recovery manifest remains valid JSON");
    let backup = manifest["backups"]
        .as_array()
        .expect("backup records are present")
        .iter()
        .find(|item| {
            item["target"]
                .as_str()
                .and_then(|path| dunce::canonicalize(path).ok())
                .as_ref()
                == Some(&target)
        })
        .and_then(|item| item["backup"].as_str())
        .expect("manifest retains the original target backup");
    assert_eq!(std::fs::read(backup).unwrap(), b"original first");
}

#[test]
fn failed_provider_rollback_helper_child() {
    let Some(root) = std::env::var_os(RECOVERY_HELPER_ROOT_ENV) else {
        return;
    };
    let root = Path::new(&root);
    let source = root.join("source");
    let target = root.join("target");
    let recovery_root = root.join("recovery");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::create_dir_all(&recovery_root).unwrap();

    let first_source = source.join("first.txt");
    let second_source = source.join("second.txt");
    let first_target = target.join("first.txt");
    let second_target = target.join("second.txt");
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
    super::super::transfer::set_recovery_base_for_test(recovery_root);

    let files = [
        first_source.to_string_lossy().into_owned(),
        second_source.to_string_lossy().into_owned(),
    ];
    let result = crate::core::fs::transfer(
        &files,
        &target.to_string_lossy(),
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
    let recovery_path = error
        .split_once("recovery retained at ")
        .map(|(_, path)| Path::new(path))
        .expect("recovery directory should be reported");
    println!(
        "RECOVERY_MANIFEST_PATH:{}",
        recovery_path.join("recovery.json").display()
    );
}

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
    let root = tempfile::tempdir().unwrap();
    let helper = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "core::fs::tests::transfer_recovery::failed_provider_rollback_helper_child",
            "--nocapture",
        ])
        .env(RECOVERY_HELPER_ROOT_ENV, root.path())
        .output()
        .unwrap();
    assert!(
        helper.status.success(),
        "transfer helper failed: {}{}",
        String::from_utf8_lossy(&helper.stdout),
        String::from_utf8_lossy(&helper.stderr)
    );

    let source = root.path().join("source");
    let target = root.path().join("target");
    let first_source = source.join("first.txt");
    let second_source = source.join("second.txt");
    let first_target = target.join("first.txt");
    let second_target = target.join("second.txt");
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

    let helper_stdout = String::from_utf8_lossy(&helper.stdout);
    let manifest_path = helper_stdout
        .lines()
        .find_map(|line| line.strip_prefix("RECOVERY_MANIFEST_PATH:"))
        .map(std::path::PathBuf::from)
        .expect("helper should report the persisted recovery manifest");
    let recovery_path = manifest_path.parent().unwrap();
    let manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let canonical_target = dunce::canonicalize(&first_target).unwrap();
    let backup = manifest["backups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            item["target"]
                .as_str()
                .and_then(|path| dunce::canonicalize(path).ok())
                .as_ref()
                == Some(&canonical_target)
        })
        .and_then(|item| item["backup"].as_str())
        .expect("original overwritten target should have a recovery backup");
    assert_eq!(std::fs::read(backup).unwrap(), b"original first");

    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "core::fs::tests::transfer_recovery::preserved_recovery_manifest_is_readable_after_process_restart",
        ])
        .env(RECOVERY_MANIFEST_ENV, recovery_path.join("recovery.json"))
        .env(RECOVERY_TARGET_ENV, &first_target)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "restart reader failed: {}{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
}
