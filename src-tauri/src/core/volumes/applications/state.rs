//! Durable Applications tree. Folder and application IDs are stable VFS
//! identities; physical paths exist only for imported assets.
mod assets;
mod entries;
mod migration;
mod mutations;
mod removal;
mod reservations;
pub(super) use assets::{applications_root_path, asset_path, imported_asset_path};
pub(super) use entries::{app_entry, entry, folder_children, folder_entry, source};
pub(super) use mutations::{
    create_folder, move_nodes, parent_uri, remove_empty_folder, remove_node, rename_folder,
    rename_imported, rename_node, validate_child, validate_remove,
};
pub(super) use reservations::{
    cancel_reservation, commit_reserved_application, commit_reserved_folder, pending_target,
    random_id, reserve_child, reserved_child,
};

pub use fiex_applications::{
    app_id, app_uri, folder_id, folder_uri, ApplicationNode, ApplicationSource, ApplicationsState,
    PendingKind, PendingTarget, VirtualFolder,
};
use fiex_applications::{app_uri_in, child_uri, folder_uri_in, parent_uri_matches};
const SCOPE: &str = fiex_applications::STORAGE_SCOPE;
const KEY: &str = fiex_applications::STATE_KEY;
pub(super) use fiex_applications::{
    valid_extension, valid_id, validate_state, APP_SEGMENT, FOLDER_SEGMENT, SCHEMA,
};

pub(super) fn storage() -> fiex_storage::native::NativeScopedStorage {
    super::super::native_vfs::management_storage_for_provider(fiex_applications::PROVIDER_ID, SCOPE)
        .expect("Applications provider registration binds its storage owner")
}

pub fn snapshot() -> Result<ApplicationsState, String> {
    ensure_ready()?;
    fiex_applications::ApplicationsStore::new(storage()).snapshot()
}

pub(super) fn ensure_ready() -> Result<(), String> {
    migration::ensure_migrated()?;
    removal::recover_pending()
}

fn update<F>(mutate: F) -> Result<ApplicationsState, String>
where
    F: FnMut(&mut ApplicationsState) -> Result<(), String>,
{
    migration::ensure_migrated()?;
    fiex_applications::ApplicationsStore::new(storage()).update(mutate)
}

pub fn is_folder(uri: &str) -> bool {
    uri == super::URI || folder_id(uri).is_some()
}

pub fn is_app(uri: &str) -> bool {
    app_id(uri).is_some()
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
        folder_id(uri)
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
    fn virtual_identity_is_opaque_and_independent_of_display_name() {
        let id = "folder-0123456789abcdef";
        let before = folder_uri(id);
        let mut node = folder(id, None);
        node.name = "Design".into();
        let after_rename = folder_uri(&node.id);
        assert_eq!(before, after_rename);
        assert_eq!(folder_id(&before).as_deref(), Some(id));
        assert!(folder_id("applications://folder_../escape").is_none());
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
            Some("applications://folder_root-folder")
        );
        assert_eq!(
            folder_uri_in(&state, "child-folder").as_deref(),
            Some("applications://folder_root-folder/folder_child-folder")
        );
        assert_eq!(
            app_uri_in(&state, "root-app").as_deref(),
            Some("applications://app_root-app")
        );
        assert_eq!(
            app_uri_in(&state, "nested-app").as_deref(),
            Some("applications://folder_root-folder/folder_child-folder/app_nested-app")
        );
        assert_eq!(
            parent_uri("applications://folder_root-folder/folder_child-folder/app_nested-app")
                .unwrap(),
            "applications://folder_root-folder/folder_child-folder"
        );
        assert!(app_id("applications://app_root-app/app_nested-app").is_none());
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
