use super::*;

fn search(root: &Path, query: &str, hidden: bool, ignores: &[String]) -> Vec<Entry> {
    let mut request = SearchRequest::new(root.to_str().unwrap(), query);
    request.hidden = hidden;
    request.ignores = ignores.to_vec();
    run(&request).0
}

/// Collects every batch so assertions cover the streamed protocol, not only
/// the final result set.
fn run(request: &SearchRequest) -> (Vec<Entry>, Vec<SearchBatch>) {
    let state = SearchState::default();
    let batches = Mutex::new(Vec::new());
    state
        .stream("test", request, |batch| {
            batches.lock().unwrap().push(batch);
        })
        .unwrap();
    let batches = batches.into_inner().unwrap();
    let entries = batches
        .iter()
        .flat_map(|batch| batch.entries.clone())
        .collect();
    (entries, batches)
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
    let (entries, batches) = run(&SearchRequest::new(t.path().to_str().unwrap(), "needle"));
    assert_eq!(entries.len(), RESULT_LIMIT);
    assert!(batches.last().unwrap().truncated);
}

#[test]
fn streams_batches_before_the_walk_finishes() {
    let t = tempfile::tempdir().unwrap();
    for index in 0..(BATCH_SIZE * 2 + 10) {
        std::fs::write(t.path().join(format!("Needle-{index}.txt")), "x").unwrap();
    }
    let (entries, batches) = run(&SearchRequest::new(t.path().to_str().unwrap(), "needle"));
    assert_eq!(entries.len(), BATCH_SIZE * 2 + 10);
    assert!(batches.len() >= 3, "{batches:?}");
    assert!(batches.iter().any(|batch| !batch.done));
    // Exactly one terminal batch, and it is the last one.
    assert_eq!(batches.iter().filter(|batch| batch.done).count(), 1);
    assert!(batches.last().unwrap().done);
}

#[test]
fn content_mode_matches_text_and_skips_binaries() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("notes.txt"), "a line with a Needle inside").unwrap();
    std::fs::write(t.path().join("binary.dat"), b"Needle\0binary").unwrap();
    std::fs::write(t.path().join("other.txt"), "nothing here").unwrap();
    let mut request = SearchRequest::new(t.path().to_str().unwrap(), "needle");
    request.mode = SearchMode::Content;
    let (entries, _) = run(&request);
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, ["notes.txt"]);
}

#[test]
fn filename_mode_does_not_read_file_content() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("plain.txt"), "needle is only in the body").unwrap();
    let entries = search(t.path(), "needle", false, &[]);
    assert!(entries.is_empty());
}

#[test]
fn explicit_engine_names_are_parsed_for_the_ui() {
    assert_eq!(SearchEngine::default(), SearchEngine::Auto);
    assert_eq!(SearchMode::default(), SearchMode::Name);
}

#[test]
fn virtual_roots_are_recognized_without_host_paths() {
    assert!(virtual_search_root("recent://"));
    assert!(virtual_search_root("applications://"));
    assert!(virtual_search_root("recyclebin://"));
    assert!(virtual_search_root("trash://"));
    assert!(virtual_search_root("applications://user/QnV6ei50eHQ"));
    assert!(!virtual_search_root("/tmp"));
    assert!(!virtual_search_root("fiex-plugin-view://example"));
    if let Some(root) = crate::core::volumes::trash_path() {
        let root = root.to_string_lossy();
        // A platform URI (the Windows Recycle Bin) is virtual, while a real
        // Unix Trash directory must never be promoted to a virtual root: the
        // Trash is only reachable through its opaque `trash://` identities.
        assert_eq!(virtual_search_root(&root), root.contains("://"), "{root}");
    }
}

#[test]
fn virtual_content_mode_warns_instead_of_returning_empty() {
    let mut request = SearchRequest::new("recent://", "needle");
    request.mode = SearchMode::Content;
    let (entries, batches) = run(&request);
    assert!(entries.is_empty());
    let last = batches.last().unwrap();
    assert!(last.done);
    assert!(last.incomplete);
    assert_eq!(last.engine.as_deref(), Some("builtin"));
    assert!(
        last.warning
            .as_deref()
            .is_some_and(|warning| warning.contains("暂不支持")),
        "{last:?}"
    );
}

#[test]
fn cold_virtual_search_finishes_without_enrolling_a_directory() {
    let request = SearchRequest::new("recent://", "__fiex_virtual_search_probe__");
    let (_entries, batches) = run(&request);
    assert!(batches.last().unwrap().done);
}
