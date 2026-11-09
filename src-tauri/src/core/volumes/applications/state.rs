//! Durable Applications tree. Internal IDs remain stable across renames while
//! VFS URI segments follow the visible folder and application names.
mod assets;
mod entries;
mod mutations;
mod removal;
mod reservations;
pub(super) use assets::asset_path;
pub(super) use entries::{app_entry, entry, folder_children, folder_entry, source};
pub(super) use fiex_applications::imported_asset_path;
pub(super) use mutations::{
    create_folder, move_nodes, parent_uri, remove_empty_folder, remove_node, rename_folder,
    rename_imported, rename_node, validate_child, validate_remove,
};
pub(super) use reservations::{
    cancel_reservation, commit_reserved_application, commit_reserved_folder, pending_target,
    random_id, reserve_child, reserved_child,
};

pub use fiex_applications::{
    ApplicationNode, ApplicationSource, ApplicationsState, PendingKind, PendingTarget,
    VirtualFolder, app_id, folder_id,
};
use fiex_applications::{app_uri_in, child_uri, folder_uri_in, parent_uri_matches};
const SCOPE: &str = fiex_applications::STORAGE_SCOPE;
const KEY: &str = fiex_applications::STATE_KEY;
#[cfg(test)]
use fiex_applications::{SCHEMA, validate_state};
pub(super) use fiex_applications::{valid_extension, valid_id};

pub(super) fn storage() -> fiex_storage::native::NativeScopedStorage {
    super::super::native_vfs::management_storage_for_provider(fiex_applications::PROVIDER_ID, SCOPE)
        .expect("Applications provider registration binds its storage owner")
}

pub fn snapshot() -> Result<ApplicationsState, String> {
    ensure_ready()?;
    fiex_applications::ApplicationsStore::new(storage()).snapshot()
}

pub(super) fn ensure_ready() -> Result<(), String> {
    removal::recover_pending()
}

fn update<F>(mutate: F) -> Result<ApplicationsState, String>
where
    F: FnMut(&mut ApplicationsState) -> Result<(), String>,
{
    fiex_applications::ApplicationsStore::new(storage()).update(mutate)
}

pub fn is_folder(uri: &str) -> bool {
    uri == super::URI || snapshot().is_ok_and(|state| folder_id(&state, uri).is_some())
}

pub fn is_app(uri: &str) -> bool {
    snapshot().is_ok_and(|state| app_id(&state, uri).is_some())
}

pub fn is_user_app(uri: &str) -> bool {
    source(uri).is_ok_and(|app| matches!(app.source, ApplicationSource::Imported { .. }))
}

pub fn remove_imported(uri: &str) -> Result<(), String> {
    removal::remove(uri)
}

fn folder_id_or_root(uri: &str) -> Result<Option<String>, String> {
    if uri == super::URI {
        Ok(None)
    } else {
        let state = snapshot()?;
        folder_id(&state, uri)
            .map(Some)
            .ok_or_else(|| "invalid Applications folder URI".into())
    }
}

fn require_parent(state: &ApplicationsState, parent: Option<&str>) -> Result<(), String> {
    if parent.is_some_and(|id| !state.folders.contains_key(id)) {
        return Err("NotFound: application parent folder".into());
    }
    Ok(())
}

fn next_order(state: &ApplicationsState, parent: Option<&str>) -> i64 {
    state
        .folders
        .values()
        .filter(|folder| folder.parent.as_deref() == parent)
        .map(|folder| folder.order)
        .chain(
            state
                .apps
                .values()
                .filter(|app| app.parent.as_deref() == parent)
                .map(|app| app.order),
        )
        .max()
        .unwrap_or(-1)
        .saturating_add(1)
}

fn ensure_unique_name(
    state: &ApplicationsState,
    parent: Option<&str>,
    name: &str,
    except_folder: Option<&str>,
) -> Result<(), String> {
    ensure_unique_name_excluding(state, parent, name, except_folder, None)
}

fn ensure_unique_name_excluding(
    state: &ApplicationsState,
    parent: Option<&str>,
    name: &str,
    except_folder: Option<&str>,
    except_app: Option<&str>,
) -> Result<(), String> {
    let same = |candidate: &str| candidate.eq_ignore_ascii_case(name);
    if state.folders.values().any(|folder| {
        folder.parent.as_deref() == parent
            && Some(folder.id.as_str()) != except_folder
            && same(&folder.name)
    }) || state.apps.values().any(|app| {
        app.parent.as_deref() == parent
            && Some(app.id.as_str()) != except_app
            && same(&app.display_name)
    }) {
        return Err(crate::i18n::t("同名项目已存在，请更换名称").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: &str, parent: Option<&str>) -> VirtualFolder {
        VirtualFolder {
            id: id.into(),
            parent: parent.map(str::to_owned),
            name: id.into(),
            order: 0,
        }
    }

    #[test]
    fn virtual_paths_follow_the_visible_folder_tree_without_type_directories() {
        let mut state = ApplicationsState {
            schema: SCHEMA,
            ..Default::default()
        };
        state
            .folders
            .insert("root-folder".into(), folder("root-folder", None));
        state.folders.insert(
            "child-folder".into(),
            folder("child-folder", Some("root-folder")),
        );
        state.apps.insert(
            "root-app".into(),
            ApplicationNode {
                id: "root-app".into(),
                parent: None,
                display_name: "Root app".into(),
                source: ApplicationSource::System {
                    stable_locator: "root-app-locator".into(),
                },
                order: 0,
                unavailable: false,
            },
        );
        state.apps.insert(
            "nested-app".into(),
            ApplicationNode {
                id: "nested-app".into(),
                parent: Some("child-folder".into()),
                display_name: "Nested app".into(),
                source: ApplicationSource::System {
                    stable_locator: "nested-app-locator".into(),
                },
                order: 0,
                unavailable: false,
            },
        );

        assert_eq!(
            folder_uri_in(&state, "root-folder").as_deref(),
            Some("applications://root-folder")
        );
        assert_eq!(
            folder_uri_in(&state, "child-folder").as_deref(),
            Some("applications://root-folder/child-folder")
        );
        assert_eq!(
            app_uri_in(&state, "root-app").as_deref(),
            Some("applications://Root%20app")
        );
        assert_eq!(
            app_uri_in(&state, "nested-app").as_deref(),
            Some("applications://root-folder/child-folder/Nested%20app")
        );
        assert_eq!(
            parent_uri("applications://root-folder/child-folder/Nested%20app").unwrap(),
            "applications://root-folder/child-folder"
        );
        assert!(app_id(&state, "applications://Root%20app/Nested%20app").is_none());
    }

    #[test]
    fn state_validation_rejects_cycles_and_missing_parents() {
        let mut state = ApplicationsState {
            schema: SCHEMA,
            ..Default::default()
        };
        state
            .folders
            .insert("folder-a".into(), folder("folder-a", Some("folder-b")));
        state
            .folders
            .insert("folder-b".into(), folder("folder-b", Some("folder-a")));
        assert!(validate_state(&state).unwrap_err().contains("cycle"));

        let mut state = ApplicationsState {
            schema: SCHEMA,
            ..Default::default()
        };
        state
            .folders
            .insert("folder-a".into(), folder("folder-a", Some("missing")));
        assert!(validate_state(&state).unwrap_err().contains("parent"));
    }
}
