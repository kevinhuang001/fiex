//! Core-owned runtime permission prompts.
//!
//! A plugin never renders an authorization dialog itself. When `host.*` finds
//! a declared capability that the user switched off, Core creates at most one
//! prompt per plugin/capability/target and blocks every waiting request on it.
//! Allowing persists the grant before the waiting operations continue; denying
//! fails the whole batch and applies a short rejection cooldown so a retrying
//! plugin cannot reopen the same dialog in a loop.
use serde::Serialize;
use std::{
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

/// Leave headroom below the 120s plugin RPC deadline so a timeout is returned
/// as an ordinary failure instead of tearing down the resident process. The
/// remaining time covers grant persistence and the actual operation.
pub(super) const WAIT_TIMEOUT: Duration = Duration::from_secs(90);
/// After an explicit denial, the same request fails without a prompt for a
/// short period. An explicit settings toggle clears this immediately.
pub(super) const REJECTION_COOLDOWN: Duration = Duration::from_secs(10);
pub(super) const MAX_PENDING: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    Permission,
    Network,
    Command,
}

impl PromptKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Permission => "permission",
            Self::Network => "network",
            Self::Command => "command",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPromptRequest {
    pub id: String,
    pub plugin_id: String,
    pub plugin_name: String,
    pub icon: Option<String>,
    pub permission: String,
    /// `permission`, `network` or `command`.
    pub kind: String,
    /// Exact domain or command for target-scoped prompts.
    pub target: Option<String>,
}

#[derive(Clone, Debug)]
pub enum PermissionPromptEvent {
    Requested(PermissionPromptRequest),
    Resolved { id: String },
}

type Sink = Box<dyn Fn(PermissionPromptEvent) + Send + Sync>;
type Grant = Box<dyn FnOnce() -> Result<(), String> + Send>;

static SINK: OnceLock<Mutex<Option<Sink>>> = OnceLock::new();

pub fn set_sink(sink: Sink) {
    let slot = SINK.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(sink);
    }
}

fn emit(event: PermissionPromptEvent) {
    let Some(slot) = SINK.get() else {
        return;
    };
    if let Ok(slot) = slot.lock() {
        if let Some(sink) = slot.as_ref() {
            sink(event);
        }
    }
}

fn prompt_available() -> bool {
    SINK.get()
        .and_then(|slot| slot.lock().ok().map(|slot| slot.is_some()))
        .unwrap_or(false)
}

mod registry;
#[cfg(test)]
mod tests;

use registry::{display, next_id, registry, resolve, wait, Key, Prompt};

pub fn request(
    plugin_id: &str,
    permission: &str,
    kind: PromptKind,
    target: Option<&str>,
    grant: Grant,
) -> Result<(), String> {
    if !prompt_available() {
        // Headless Core and tests have no UI to answer a prompt. Fail closed
        // instead of blocking the plugin for the whole timeout.
        return Err("PermissionDenied: permission prompt is unavailable".into());
    }
    let key = Key::new(plugin_id, permission, kind, target)?;
    // Resolve display metadata before taking the registry lock: catalog reads
    // the manifest and grant store and must never block other prompts.
    let (plugin_name, icon) = display(plugin_id);
    let (prompt, created) = {
        let mut registry = registry()
            .lock()
            .map_err(|_| "permission prompt registry unavailable")?;
        let now = Instant::now();
        registry
            .rejected
            .retain(|_, at| now.duration_since(*at) < REJECTION_COOLDOWN);
        if registry.rejected.contains_key(&key) {
            return Err(format!(
                "PermissionDenied: {permission} was denied recently; allow it from plugin settings or retry later"
            ));
        }
        if let Some(prompt) = registry.pending.get(&key) {
            (Arc::clone(prompt), false)
        } else {
            if registry.pending.len() >= MAX_PENDING {
                return Err("PermissionDenied: too many pending permission requests".into());
            }
            let id = next_id();
            let prompt = Arc::new(Prompt {
                id: id.clone(),
                key: key.clone(),
                request: PermissionPromptRequest {
                    id,
                    plugin_id: plugin_id.to_string(),
                    plugin_name,
                    icon,
                    permission: permission.to_string(),
                    kind: kind.as_str().to_string(),
                    target: target.map(str::to_owned),
                },
                grant: Mutex::new(Some(grant)),
                state: Mutex::new(None),
                wake: Condvar::new(),
            });
            registry.pending.insert(key, Arc::clone(&prompt));
            (prompt, true)
        }
    };
    if created {
        emit(PermissionPromptEvent::Requested(prompt.request.clone()));
    }
    wait(&prompt)
}

/// The UI answer. Core, not the plugin, owns persistence: allowing runs the
/// grant closure captured when the request was created and only then unblocks
/// the waiting operations.
pub fn respond(id: &str, allow: bool) -> Result<(), String> {
    let prompt = {
        let registry = registry()
            .lock()
            .map_err(|_| "permission prompt registry unavailable")?;
        registry
            .pending
            .values()
            .find(|prompt| prompt.id == id)
            .cloned()
            .ok_or("permission request expired")?
    };
    let grant = prompt
        .grant
        .lock()
        .map_err(|_| "permission prompt unavailable")?
        .take();
    let Some(grant) = grant else {
        return Err("permission request was already resolved".into());
    };
    if allow {
        let result = match grant() {
            Ok(()) => Ok(()),
            Err(error) => Err(format!(
                "PermissionDenied: could not save permission: {error}"
            )),
        };
        resolve(&prompt, result.clone(), false);
        result
    } else {
        // Denying is a successful answer from the UI's point of view. The
        // waiting operations receive the denial below.
        resolve(
            &prompt,
            Err(format!(
                "PermissionDenied: user denied {} for this operation",
                prompt.request.permission
            )),
            true,
        );
        Ok(())
    }
}

/// Stop waiting requests when the plugin or its process goes away. The guard
/// in Core does not keep a dead process's dialogs on screen.
pub fn cancel_plugin(plugin_id: &str) {
    let prompts = {
        let Ok(registry) = registry().lock() else {
            return;
        };
        registry
            .pending
            .iter()
            .filter(|(key, _)| key.plugin == plugin_id)
            .map(|(_, prompt)| Arc::clone(prompt))
            .collect::<Vec<_>>()
    };
    for prompt in prompts {
        resolve(
            &prompt,
            Err("PermissionDenied: plugin stopped while waiting for permission".into()),
            false,
        );
    }
}

/// An explicit user grant from settings supersedes a previous runtime denial.
pub fn clear_rejection(plugin_id: &str, permission: &str) {
    if let Ok(mut registry) = registry().lock() {
        registry
            .rejected
            .retain(|key, _| key.plugin != plugin_id || key.permission != permission);
    }
}
