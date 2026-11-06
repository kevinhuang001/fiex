use super::*;
use crate::{ApplicationNode, ApplicationSource, VirtualFolder};

fn fixture() -> ApplicationsState {
    let mut state = ApplicationsState {
        schema: crate::SCHEMA,
        ..Default::default()
    };
    state.folders.insert(
        "source".into(),
        VirtualFolder {
            id: "source".into(),
            parent: None,
            name: "Source".into(),
            order: 0,
        },
    );
    state.folders.insert(
        "target".into(),
        VirtualFolder {
            id: "target".into(),
            parent: None,
            name: "Target".into(),
            order: 1,
        },
    );
    state.apps.insert(
        "app".into(),
        ApplicationNode {
            id: "app".into(),
            parent: Some("source".into()),
            display_name: "Editor.exe".into(),
            source: ApplicationSource::Imported {
                asset_id: "asset".into(),
                extension: "exe".into(),
            },
            order: 0,
            unavailable: false,
        },
    );
    state
}

#[test]
fn relocation_changes_only_the_virtual_tree_parent() {
    let mut state = fixture();
    let old_uri = app_uri_in(&state, "app").unwrap();
    let target = folder_uri_in(&state, "target").unwrap();
    let moved = move_nodes_in_state(&mut state, &[old_uri], &target).unwrap();
    assert_eq!(state.apps["app"].parent.as_deref(), Some("target"));
    assert_eq!(moved, vec![app_uri_in(&state, "app").unwrap()]);
    assert_eq!(state.apps["app"].source, fixture().apps["app"].source);
}

#[test]
fn moving_into_a_descendant_is_rejected_without_mutation() {
    let mut state = fixture();
    state.folders.insert(
        "nested".into(),
        VirtualFolder {
            id: "nested".into(),
            parent: Some("source".into()),
            name: "Nested".into(),
            order: 1,
        },
    );
    let before = state.clone();
    let target = folder_uri_in(&state, "nested").unwrap();
    let source = folder_uri_in(&state, "source").unwrap();
    let error = move_nodes_in_state(&mut state, &[source], &target).unwrap_err();
    assert_eq!(error, "Cannot move a folder into its descendant");
    assert_eq!(state, before);
}

#[test]
fn same_parent_is_a_no_op_and_child_validation_uses_canonical_uri() {
    let mut state = fixture();
    let source = folder_uri_in(&state, "source").unwrap();
    let app = app_uri_in(&state, "app").unwrap();
    let before = state.clone();
    assert_eq!(
        move_nodes_in_state(&mut state, std::slice::from_ref(&app), &source).unwrap(),
        vec![app.clone()]
    );
    assert_eq!(state, before);
    validate_child_in(&state, &source, &app).unwrap();
    assert!(validate_child_in(&state, crate::URI, &app).is_err());
}
