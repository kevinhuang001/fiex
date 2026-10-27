use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::Instant,
};

use super::{emit, PermissionPromptEvent, PermissionPromptRequest, PromptKind, WAIT_TIMEOUT};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Key {
    pub(super) plugin: String,
    pub(super) permission: String,
    pub(super) target: Option<String>,
}

impl Key {
    pub(super) fn new(
        plugin_id: &str,
        permission: &str,
        kind: PromptKind,
        target: Option<&str>,
    ) -> Result<Self, String> {
        let target = match kind {
            PromptKind::Permission => None,
            PromptKind::Network | PromptKind::Command => {
                let target = target
                    .filter(|value| !value.is_empty())
                    .ok_or("permission prompt target is required")?;
                Some(target.to_ascii_lowercase())
            }
        };
        Ok(Self {
            plugin: plugin_id.to_string(),
            permission: permission.to_string(),
            target,
        })
    }
}

pub(super) struct Prompt {
    pub(super) id: String,
    pub(super) key: Key,
    pub(super) request: PermissionPromptRequest,
    pub(super) grant: Mutex<Option<super::Grant>>,
    pub(super) state: Mutex<Option<Result<(), String>>>,
    pub(super) wake: Condvar,
}

#[derive(Default)]
pub(super) struct Registry {
    pub(super) pending: HashMap<Key, Arc<Prompt>>,
    pub(super) rejected: HashMap<Key, Instant>,
}

pub(super) fn registry() -> &'static Mutex<Registry> {
    static VALUES: OnceLock<Mutex<Registry>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(Registry::default()))
}

pub(super) fn next_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "permission-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

pub(super) fn display(plugin_id: &str) -> (String, Option<String>) {
    crate::plugins::host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == plugin_id)
        .map(|plugin| (plugin.name, plugin.icon))
        .unwrap_or_else(|| (plugin_id.to_string(), None))
}

/// Ask the user for one capability. Requests with the same key share a single
/// visible prompt; every waiter receives the same decision. `grant` runs on
/// the allow path before waiters continue, so the persisted state is visible
/// to the next operation.
pub(super) fn wait(prompt: &Arc<Prompt>) -> Result<(), String> {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    let mut state = prompt
        .state
        .lock()
        .map_err(|_| "permission prompt state unavailable")?;
    loop {
        if let Some(result) = state.as_ref() {
            return result.clone();
        }
        let now = Instant::now();
        if now >= deadline {
            drop(state);
            resolve(
                prompt,
                Err("PermissionDenied: permission request timed out".into()),
                false,
            );
            return Err("PermissionDenied: permission request timed out".into());
        }
        let (next, _) = prompt
            .wake
            .wait_timeout(state, deadline - now)
            .map_err(|_| "permission prompt wait failed")?;
        state = next;
    }
}

pub(super) fn resolve(prompt: &Arc<Prompt>, result: Result<(), String>, rejected: bool) {
    let resolved = {
        let mut registry = match registry().lock() {
            Ok(registry) => registry,
            Err(_) => return,
        };
        if registry
            .pending
            .get(&prompt.key)
            .is_some_and(|current| Arc::ptr_eq(current, prompt))
        {
            registry.pending.remove(&prompt.key);
        }
        if rejected {
            registry.rejected.insert(prompt.key.clone(), Instant::now());
        }
        // Invalidate a concurrent respond; the decision below is authoritative.
        if let Ok(mut grant) = prompt.grant.lock() {
            grant.take();
        }
        let mut state = match prompt.state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if state.is_some() {
            false
        } else {
            *state = Some(result);
            true
        }
    };
    prompt.wake.notify_all();
    if resolved {
        emit(PermissionPromptEvent::Resolved {
            id: prompt.id.clone(),
        });
    }
}
