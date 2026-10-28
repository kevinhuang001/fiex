use super::*;
use crate::{
    core::operations::Operations,
    core::transfers::{Action, NEVER_CANCELLED},
};

fn resolve(result: &OperationResult, action: Action) -> Vec<Resolution> {
    result
        .conflicts
        .iter()
        .map(|c| Resolution {
            key: c.key.clone(),
            action,
        })
        .collect()
}

#[test]
fn folder_progress_counts_each_file() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("source");
    let destination = t.path().join("destination");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::create_dir(&destination).unwrap();
    for name in ["one.txt", "nested/two.txt", "nested/three.txt"] {
        fs::write(source.join(name), name).unwrap();
    }
    let samples = std::sync::Mutex::new(Vec::new());
    execute(
        &[source.to_string_lossy().into_owned()],
        destination.to_str().unwrap(),
        false,
        false,
        &[],
        &[],
        &NEVER_CANCELLED,
        &|done, total, _| samples.lock().unwrap().push((done, total)),
    )
    .unwrap();
    let samples = samples.into_inner().unwrap();
    assert!(samples.iter().all(|(_, total)| *total == 3), "{samples:?}");
    assert_eq!(samples.last(), Some(&(3, 3)));
}

#[test]
fn conflicts_are_preflighted_replace_is_undoable_and_stale_choices_are_rejected() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a");
    let b = t.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::write(a.join("x.txt"), "source").unwrap();
    fs::write(a.join("new.txt"), "new").unwrap();
    fs::write(b.join("x.txt"), "original").unwrap();
    let sources = vec![
        a.join("x.txt").to_string_lossy().into_owned(),
        a.join("new.txt").to_string_lossy().into_owned(),
    ];
    let op = Operations::default();
    let first = op
        .transfer(
            &sources,
            b.to_str().unwrap(),
            true,
            false,
            &[],
            &NEVER_CANCELLED,
        )
        .unwrap();
    assert_eq!(first.conflicts.len(), 1);
    assert!(!b.join("new.txt").exists());
    let choices = resolve(&first, Action::Replace);
    fs::write(b.join("x.txt"), "changed since prompt").unwrap();
    let stale = op
        .transfer_resolved(
            &sources,
            b.to_str().unwrap(),
            true,
            false,
            &[],
            &choices,
            &NEVER_CANCELLED,
        )
        .unwrap();
    assert_eq!(stale.conflicts.len(), 1);
    let result = op
        .transfer_resolved(
            &sources,
            b.to_str().unwrap(),
            true,
            false,
            &[],
            &resolve(&stale, Action::Replace),
            &NEVER_CANCELLED,
        )
        .unwrap();
    assert!(result.errors.is_empty());
    assert!(!a.join("x.txt").exists());
    assert_eq!(fs::read_to_string(b.join("x.txt")).unwrap(), "source");
    op.undo().unwrap();
    assert_eq!(
        fs::read_to_string(b.join("x.txt")).unwrap(),
        "changed since prompt"
    );
    assert_eq!(fs::read_to_string(a.join("x.txt")).unwrap(), "source");
}

#[test]
fn merging_folders_preserves_skipped_files_and_keep_both_keeps_extension() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a");
    let b = t.path().join("b");
    fs::create_dir_all(a.join("folder")).unwrap();
    fs::create_dir_all(b.join("folder")).unwrap();
    fs::write(a.join("folder/x.txt"), "source").unwrap();
    fs::write(a.join("folder/new.txt"), "new").unwrap();
    fs::write(b.join("folder/x.txt"), "existing").unwrap();
    let source = vec![a.join("folder").to_string_lossy().into_owned()];
    let op = Operations::default();
    let first = op
        .transfer(
            &source,
            b.to_str().unwrap(),
            true,
            false,
            &[],
            &NEVER_CANCELLED,
        )
        .unwrap();
    op.transfer_resolved(
        &source,
        b.to_str().unwrap(),
        true,
        false,
        &[],
        &resolve(&first, Action::Skip),
        &NEVER_CANCELLED,
    )
    .unwrap();
    assert!(a.join("folder/x.txt").exists());
    assert!(!a.join("folder/new.txt").exists());
    let next = op
        .transfer(
            &source,
            b.to_str().unwrap(),
            false,
            false,
            &[],
            &NEVER_CANCELLED,
        )
        .unwrap();
    op.transfer_resolved(
        &source,
        b.to_str().unwrap(),
        false,
        false,
        &[],
        &resolve(&next, Action::KeepBoth),
        &NEVER_CANCELLED,
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(b.join("folder/x (2).txt")).unwrap(),
        "source"
    );
    assert_eq!(
        fs::read_to_string(b.join("folder/x.txt")).unwrap(),
        "existing"
    );
}
