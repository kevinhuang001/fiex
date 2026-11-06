use super::*;
use crate::{app_id, ApplicationDecorationInfo};
use fiex_storage::{native::OwnerBoundStorage, CoreOwnerKey};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestBackend {
    calls: AtomicUsize,
    applications: Mutex<Vec<InstalledApplication>>,
}

impl ApplicationsBackend for TestBackend {
    fn enumerate_installed(&self) -> Result<Vec<InstalledApplication>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.applications.lock().unwrap().clone())
    }

    fn open_system_application(&self, _: &str) -> Result<(), String> {
        Ok(())
    }

    fn classify_host_application(&self, _: &std::path::Path, _: Option<&str>) -> bool {
        false
    }

    fn is_user_application(&self, _: &std::path::Path) -> bool {
        false
    }

    fn validate_import_asset(&self, _: &std::path::Path, _: &str) -> Result<(), String> {
        Ok(())
    }
}

fn store(owner: &'static str) -> ApplicationsStore {
    let storage = OwnerBoundStorage::from_registry(CoreOwnerKey::new(owner))
        .scoped("tree")
        .unwrap();
    storage.delete("state").unwrap();
    ApplicationsStore::new(storage)
}

fn application(name: &str) -> InstalledApplication {
    InstalledApplication {
        stable_locator: "/system/Editor.app".into(),
        display_name: name.into(),
        icon_source: Some("/system/Editor.app".into()),
        extension: "app".into(),
        modified: 42,
    }
}

#[test]
fn catalog_caches_reads_and_preserves_stable_identity_across_refreshes() {
    let store = store("applications-catalog-refresh-test");
    let backend = Arc::new(TestBackend {
        calls: AtomicUsize::new(0),
        applications: Mutex::new(vec![application("Editor")]),
    });
    let catalog = ApplicationCatalog::new(store.clone(), backend.clone());
    let initial = catalog.entries().unwrap();
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    assert_eq!(catalog.entries().unwrap(), initial);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);

    backend.applications.lock().unwrap()[0] = application("Editor Renamed");
    let refreshed = catalog.refresh().unwrap();
    assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
    assert_ne!(refreshed[0].uri, initial[0].uri);
    assert_eq!(initial[0].uri, "applications://Editor");
    assert_eq!(refreshed[0].uri, "applications://Editor%20Renamed");
    assert_eq!(refreshed[0].display_name, "Editor Renamed");
    let persisted = store.snapshot().unwrap();
    let id = app_id(&persisted, &refreshed[0].uri).unwrap();
    assert_eq!(persisted.apps[&id].display_name, "Editor Renamed");
}

#[test]
fn missing_system_app_is_retained_as_unavailable_with_its_identity() {
    let store = store("applications-catalog-unavailable-test");
    let backend = Arc::new(TestBackend {
        calls: AtomicUsize::new(0),
        applications: Mutex::new(vec![application("Editor")]),
    });
    let catalog = ApplicationCatalog::new(store.clone(), backend.clone());
    let uri = catalog.entries().unwrap()[0].uri.clone();
    backend.applications.lock().unwrap().clear();
    assert!(catalog.refresh().unwrap().is_empty());

    let state = store.snapshot().unwrap();
    let id = app_id(&state, &uri).unwrap();
    assert!(state.apps[&id].unavailable);
    assert!(app_uri_in(&state, &id).is_some());
}

#[test]
fn system_application_identity_depends_only_on_the_stable_locator() {
    assert_eq!(
        system_id("/Applications/Editor.app"),
        system_id("/Applications/Editor.app")
    );
    assert_ne!(
        system_id("/Applications/Editor.app"),
        system_id("/Applications/Other.app")
    );
}

#[test]
fn decoration_info_tracks_system_apps_after_they_move_into_virtual_folders() {
    let store = store("applications-catalog-decoration-test");
    let backend = Arc::new(TestBackend {
        calls: AtomicUsize::new(0),
        applications: Mutex::new(vec![application("Editor")]),
    });
    let catalog = ApplicationCatalog::new(store.clone(), backend);
    let original = catalog.entries().unwrap()[0].uri.clone();
    let id = app_id(&store.snapshot().unwrap(), &original).unwrap();

    let tree = store
        .update(|tree| {
            tree.folders.insert(
                "tools".into(),
                crate::VirtualFolder {
                    id: "tools".into(),
                    parent: None,
                    name: "Tools".into(),
                    order: 0,
                },
            );
            tree.apps.get_mut(&id).unwrap().parent = Some("tools".into());
            Ok(())
        })
        .unwrap();
    let moved = app_uri_in(&tree, &id).unwrap();
    let decoration = catalog.decoration_info(&[original.clone(), moved.clone()]);

    assert!(!decoration.contains_key(&original));
    assert_eq!(
        decoration.get(&moved),
        Some(&ApplicationDecorationInfo {
            is_system: true,
            icon_source: Some("/system/Editor.app".into()),
        })
    );
}
