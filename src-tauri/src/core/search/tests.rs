use super::*;

fn search(root: &Path, query: &str, hidden: bool, ignores: &[String]) -> Vec<Entry> {
    let state = SearchState::default();
    let found = Mutex::new(Vec::new());
    state
        .run(
            "test",
            root.to_str().unwrap(),
            query,
            hidden,
            ignores,
            |batch| found.lock().unwrap().extend(batch.entries),
        )
        .unwrap();
    found.into_inner().unwrap()
}

#[test]
fn finds_app_bundles_without_entering_them() {
    let t = tempfile::tempdir().unwrap();
    let bundle = t.path().join("Safari.app");
    std::fs::create_dir_all(bundle.join("Contents")).unwrap();
    std::fs::write(bundle.join("Contents").join("Needle.plist"), "x").unwrap();
    std::fs::write(t.path().join("Needle.txt"), "x").unwrap();

    let found = search(t.path(), "needle", false, &[]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Needle.txt");

    let apps = search(t.path(), "safari", false, &[]);
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "Safari.app");
    assert!(apps[0].is_dir);
}

#[test]
fn finds_hidden_when_requested() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join(".Needle.txt"), "x").unwrap();
    assert_eq!(search(t.path(), "needle", false, &[]).len(), 0);
    assert_eq!(search(t.path(), "needle", true, &[]).len(), 1);
}

#[test]
fn display_ignores_filter_results() {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join("node_modules")).unwrap();
    std::fs::write(t.path().join("node_modules").join("Needle.js"), "x").unwrap();
    std::fs::write(t.path().join("Needle.txt"), "x").unwrap();
    let found = search(t.path(), "needle", false, &["node_modules".into()]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Needle.txt");
}

#[test]
fn live_results_follow_filesystem_changes() {
    let t = tempfile::tempdir().unwrap();
    let folder = t.path().join("OldFolder");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("Needle.txt"), "x").unwrap();
    assert_eq!(search(t.path(), "needle", false, &[]).len(), 1);
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(search(t.path(), "needle", false, &[]).len(), 0);
}

#[test]
fn stops_at_the_result_limit() {
    let t = tempfile::tempdir().unwrap();
    for index in 0..(RESULT_LIMIT + 20) {
        std::fs::write(t.path().join(format!("Needle-{index}.txt")), "x").unwrap();
    }
    let state = SearchState::default();
    let result = Mutex::new((0usize, false));
    state
        .run(
            "test",
            t.path().to_str().unwrap(),
            "needle",
            false,
            &[],
            |batch| {
                let mut result = result.lock().unwrap();
                result.0 = batch.entries.len();
                result.1 = batch.truncated;
            },
        )
        .unwrap();
    let (count, truncated) = result.into_inner().unwrap();
    assert_eq!(count, RESULT_LIMIT);
    assert!(truncated);
}

#[test]
fn local_stream_emits_hits_before_completion() {
    let folder = tempfile::tempdir().unwrap();
    for index in 0..55 {
        std::fs::write(folder.path().join(format!("needle-{index}.txt")), "body").unwrap();
    }
    let batches = Mutex::new(Vec::new());
    search_local_stream(folder.path(), "needle", "name", false, &[], &|| false,
        |batch| batches.lock().unwrap().push((batch.entries.len(), batch.done))).unwrap();
    let batches = batches.into_inner().unwrap();
    assert!(batches.len() >= 2);
    assert_eq!(batches[0], (50, false));
    assert_eq!(batches.last().unwrap().1, true);
    assert_eq!(batches.iter().map(|(count, _)| count).sum::<usize>(), 55);
}
