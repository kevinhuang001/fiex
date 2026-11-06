//! Application discovery, stable identities, and platform boundary.
use crate::{app_uri_in, ApplicationNode, ApplicationSource, ApplicationsState, ApplicationsStore};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledApplication {
    pub stable_locator: String,
    pub display_name: String,
    pub icon_source: Option<String>,
    pub extension: String,
    pub modified: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogApplication {
    pub(crate) id: String,
    pub uri: String,
    pub display_name: String,
    pub icon_source: Option<String>,
    pub extension: String,
    pub modified: u64,
}

/// OS operations are injected by the application composition root.
pub trait ApplicationsBackend: Send + Sync {
    fn enumerate_installed(&self) -> Result<Vec<InstalledApplication>, String>;
    fn open_system_application(&self, stable_locator: &str) -> Result<(), String>;
    fn classify_host_application(&self, path: &std::path::Path, target: Option<&str>) -> bool;
    fn is_user_application(&self, path: &std::path::Path) -> bool;
    fn validate_import_asset(
        &self,
        path: &std::path::Path,
        display_name: &str,
    ) -> Result<(), String>;
}

#[derive(Default)]
pub(super) struct CatalogState {
    running: bool,
    pub(super) cached: Option<Vec<CatalogApplication>>,
    last_result: Option<Result<Vec<CatalogApplication>, String>>,
    refreshed_at: Option<Instant>,
}

/// Caches installed applications while persisting stable internal identities.
pub struct ApplicationCatalog {
    pub(super) store: ApplicationsStore,
    backend: Arc<dyn ApplicationsBackend>,
    pub(super) state: Mutex<CatalogState>,
    completed: Condvar,
}

impl ApplicationCatalog {
    pub fn new(store: ApplicationsStore, backend: Arc<dyn ApplicationsBackend>) -> Self {
        Self {
            store,
            backend,
            state: Mutex::new(CatalogState::default()),
            completed: Condvar::new(),
        }
    }

    /// Force an OS enumeration; concurrent callers share its result.
    pub fn refresh(&self) -> Result<Vec<CatalogApplication>, String> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.running {
            while state.running {
                state = self
                    .completed
                    .wait(state)
                    .unwrap_or_else(|error| error.into_inner());
            }
            return state
                .last_result
                .clone()
                .unwrap_or_else(|| Err("application enumeration did not produce a result".into()));
        }
        state.running = true;
        drop(state);

        let result = self.enumerate_and_register();
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.last_result = Some(result.clone());
        if let Ok(applications) = &result {
            state.cached = Some(applications.clone());
        }
        state.refreshed_at = Some(Instant::now());
        state.running = false;
        self.completed.notify_all();
        result
    }

    /// Return cached entries when available, enumerating only on the first read.
    pub fn entries(&self) -> Result<Vec<CatalogApplication>, String> {
        if let Some(applications) = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cached
            .clone()
        {
            return Ok(applications);
        }
        self.refresh()
    }

    /// Return the last durable snapshot immediately, then revalidate installed
    /// applications off the caller's thread. `on_change` runs only when the
    /// visible catalogue changes, so callers can refresh an open Applications
    /// view without restarting its loading state on every enumeration.
    pub fn entries_cache_first_background(
        self: &Arc<Self>,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<Vec<CatalogApplication>, String> {
        let has_cache = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cached
            .is_some();
        let cached = if has_cache {
            None
        } else {
            Some(self.seed_from_store()?)
        };
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.cached.is_none() {
            state.cached = cached;
        }
        let entries = state.cached.clone().unwrap_or_default();
        let refresh_due = state
            .refreshed_at
            .is_none_or(|at| at.elapsed() >= REFRESH_INTERVAL);
        if !refresh_due || state.running {
            return Ok(entries);
        }
        state.running = true;
        let catalog = Arc::clone(self);
        drop(state);
        let spawn = std::thread::Builder::new()
            .name("fiex-applications-refresh".into())
            .spawn(move || {
                let result = catalog.enumerate_and_register();
                let mut state = catalog
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let changed = match &result {
                    Ok(applications) => {
                        let changed = state
                            .cached
                            .as_ref()
                            .is_none_or(|cached| !same_visible_catalog(cached, applications));
                        state.cached = Some(applications.clone());
                        changed
                    }
                    Err(_) => false,
                };
                state.refreshed_at = Some(Instant::now());
                state.last_result = Some(result);
                state.running = false;
                catalog.completed.notify_all();
                drop(state);
                if changed {
                    on_change();
                }
            });
        if let Err(error) = spawn {
            self.state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .running = false;
            return Err(error.to_string());
        }
        Ok(entries)
    }

    pub fn open_system_application(&self, stable_locator: &str) -> Result<(), String> {
        self.backend.open_system_application(stable_locator)
    }

    pub fn classify_host_application(&self, path: &std::path::Path, target: Option<&str>) -> bool {
        self.backend.classify_host_application(path, target)
    }

    pub fn is_user_application(&self, path: &std::path::Path) -> bool {
        self.backend.is_user_application(path)
    }

    pub fn validate_import_asset(
        &self,
        path: &std::path::Path,
        display_name: &str,
    ) -> Result<(), String> {
        self.backend.validate_import_asset(path, display_name)
    }

    fn enumerate_and_register(&self) -> Result<Vec<CatalogApplication>, String> {
        let discovered = self.backend.enumerate_installed()?;
        let mut records = discovered
            .iter()
            .map(|application| {
                (
                    application.stable_locator.clone(),
                    application.display_name.clone(),
                )
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.0.cmp(&right.0));
        let identities = self.store.register_system_apps(&records)?;
        let tree = self.store.snapshot()?;
        discovered
            .into_iter()
            .map(|application| {
                let id = system_id(&application.stable_locator);
                let uri = identities
                    .get(&application.stable_locator)
                    .cloned()
                    .ok_or("system application identity was not persisted")?;
                let display_name = tree
                    .apps
                    .get(&id)
                    .map(|app| app.display_name.clone())
                    .ok_or("system application state was not persisted")?;
                Ok(CatalogApplication {
                    id,
                    uri,
                    display_name,
                    icon_source: application.icon_source,
                    extension: application.extension,
                    modified: application.modified,
                })
            })
            .collect()
    }

    fn seed_from_store(&self) -> Result<Vec<CatalogApplication>, String> {
        let state = self.store.snapshot()?;
        state
            .apps
            .values()
            .filter(|app| matches!(&app.source, ApplicationSource::System { .. }))
            .map(|app| {
                let ApplicationSource::System { stable_locator } = &app.source else {
                    unreachable!("system application filter checked the source")
                };
                Some(CatalogApplication {
                    id: app.id.clone(),
                    uri: app_uri_in(&state, &app.id)?,
                    display_name: app.display_name.clone(),
                    icon_source: Some(stable_locator.clone()),
                    extension: Path::new(&app.display_name)
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase(),
                    modified: 0,
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "application state contains an invalid tree".to_owned())
    }
}

fn same_visible_catalog(left: &[CatalogApplication], right: &[CatalogApplication]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.id == right.id
                && left.uri == right.uri
                && left.display_name == right.display_name
                && left.icon_source == right.icon_source
                && left.extension == right.extension
        })
}

impl ApplicationsStore {
    pub fn register_system_apps(
        &self,
        records: &[(String, String)],
    ) -> Result<std::collections::BTreeMap<String, String>, String> {
        let identities = records
            .iter()
            .map(|(locator, _)| (system_id(locator), locator.clone()))
            .collect::<Vec<_>>();
        let state = self.update(|state| {
            let incoming = identities
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<HashSet<_>>();
            let mut names = state
                .folders
                .values()
                .map(|folder| (folder.parent.clone(), folder.name.to_lowercase()))
                .chain(
                    state
                        .apps
                        .values()
                        .filter(|app| !incoming.contains(app.id.as_str()))
                        .map(|app| (app.parent.clone(), app.display_name.to_lowercase())),
                )
                .collect::<HashSet<_>>();
            for app in state.apps.values_mut() {
                if matches!(app.source, ApplicationSource::System { .. }) {
                    app.unavailable = true;
                }
            }
            for ((_, name), (id, locator)) in records.iter().zip(identities.iter()) {
                let parent = state.apps.get(id).and_then(|app| app.parent.clone());
                let unique_name = unique_name(&safe_name(name), &parent, &mut names);
                match state.apps.get_mut(id) {
                    Some(app) => {
                        app.display_name = unique_name;
                        app.unavailable = false;
                    }
                    None => {
                        let order = next_order(state, None);
                        state.apps.insert(
                            id.clone(),
                            ApplicationNode {
                                id: id.clone(),
                                parent: None,
                                display_name: unique_name,
                                source: ApplicationSource::System {
                                    stable_locator: locator.clone(),
                                },
                                order,
                                unavailable: false,
                            },
                        );
                    }
                }
            }
            Ok(())
        })?;
        Ok(identities
            .into_iter()
            .map(|(id, locator)| {
                let uri = app_uri_in(&state, &id).expect("system application was inserted");
                (locator, uri)
            })
            .collect())
    }
}

fn unique_name(
    desired: &str,
    parent: &Option<String>,
    names: &mut HashSet<(Option<String>, String)>,
) -> String {
    let mut candidate = desired.to_owned();
    let mut suffix = 2_u32;
    while names.contains(&(parent.clone(), candidate.to_lowercase())) {
        candidate = format!("{desired} ({suffix})");
        suffix += 1;
    }
    names.insert((parent.clone(), candidate.to_lowercase()));
    candidate
}

fn safe_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|character| match character {
            '/' => '／',
            '\\' => '＼',
            character if character.is_control() => '�',
            character => character,
        })
        .collect::<String>();
    if name.trim().is_empty() {
        "Unnamed application".into()
    } else {
        name
    }
}

pub fn system_id(locator: &str) -> String {
    format!("sys-{}", hex::encode(Sha256::digest(locator.as_bytes())))
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

#[cfg(test)]
#[path = "catalog/tests.rs"]
mod tests;
