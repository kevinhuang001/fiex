use super::*;
use crate::core::operations::{CANCELLED, OperationCancel};

#[test]
fn transfers_never_overwrite() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a");
    let b = t.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::write(a.join("x"), "source").unwrap();
    fs::write(b.join("x"), "existing").unwrap();
    let op = Operations::default();
    assert_eq!(
        op.transfer(
            &[a.join("x").to_string_lossy().into()],
            b.to_str().unwrap(),
            false,
            false,
            &[],
            &AtomicBool::new(false)
        )
        .unwrap()
        .conflicts
        .len(),
        1
    );
    op.transfer(
        &[a.join("x").to_string_lossy().into()],
        b.to_str().unwrap(),
        false,
        true,
        &[],
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(fs::read_to_string(b.join("x")).unwrap(), "existing");
    assert!(b.join("x (1)").exists());
}

#[test]
fn recursive_and_traversal_rejected() {
    let t = tempfile::tempdir().unwrap();
    let op = Operations::default();
    assert!(
        op.create(t.path().to_str().unwrap(), "../escape", true)
            .is_err()
    );
    assert!(
        op.transfer(
            &[t.path().to_string_lossy().into()],
            t.path().to_str().unwrap(),
            false,
            true,
            &[],
            &AtomicBool::new(false)
        )
        .is_err()
    );
}

#[test]
fn multiple_selected_files_copy_through_the_parallel_path() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("a");
    let dest = t.path().join("b");
    fs::create_dir(&src).unwrap();
    fs::create_dir(&dest).unwrap();
    let sources = (0..32)
        .map(|index| {
            let path = src.join(format!("file-{index}.txt"));
            fs::write(&path, format!("content-{index}")).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect::<Vec<_>>();
    let op = Operations::default();
    let result = op
        .transfer(
            &sources,
            dest.to_str().unwrap(),
            false,
            false,
            &[],
            &AtomicBool::new(false),
        )
        .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.paths.len(), 32);
    for index in 0..32 {
        assert_eq!(
            fs::read_to_string(dest.join(format!("file-{index}.txt"))).unwrap(),
            format!("content-{index}")
        );
    }
}

#[test]
fn cancelled_transfer_stops_before_moving_any_file() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a");
    let b = t.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::write(a.join("one"), "first").unwrap();
    fs::write(a.join("two"), "second").unwrap();
    let sources = vec![
        a.join("one").to_string_lossy().into_owned(),
        a.join("two").to_string_lossy().into_owned(),
    ];
    let cancel = OperationCancel::default();
    cancel.cancel();
    let op = Operations::default();
    assert_eq!(
        op.transfer(
            &sources,
            b.to_str().unwrap(),
            true,
            false,
            &[],
            &cancel.flag()
        )
        .err(),
        Some(CANCELLED.to_string())
    );
    assert!(!b.join("one").exists());
    assert!(!b.join("two").exists());
    assert!(a.join("one").exists());
    assert!(a.join("two").exists());
}

#[test]
fn rename_undo() {
    let t = tempfile::tempdir().unwrap();
    let op = Operations::default();
    fs::write(t.path().join("a"), "x").unwrap();
    op.rename(t.path().join("a").to_str().unwrap(), "b")
        .unwrap();
    op.undo().unwrap();
    assert!(t.path().join("a").exists());
}
