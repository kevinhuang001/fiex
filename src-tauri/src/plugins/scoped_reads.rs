//! Opaque read capabilities handed to standard events.
//!
//! A capability binds one already-authorized resource to one plugin instance
//! and event session. Opening it still goes through the operation scope that
//! Core attaches to the invocation; the registry never stores a host path that
//! a plugin could guess.
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const TTL: Duration = Duration::from_secs(30 * 60);
const MAX_ENTRIES: usize = 256;

struct Entry {
    plugin_id: String,
    session_id: String,
    name: String,
    created: Instant,
}

fn registry() -> &'static Mutex<HashMap<String, Entry>> {
    static VALUES: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register(plugin_id: &str, session_id: &str, name: &str) -> String {
    let id = format!("fiex-read:{}", crate::mounts::new_id().unwrap_or_default());
    if let Ok(mut values) = registry().lock() {
        values.retain(|_, entry| entry.created.elapsed() < TTL);
        if values.len() >= MAX_ENTRIES {
            if let Some(oldest) = values
                .iter()
                .min_by_key(|(_, entry)| entry.created)
                .map(|(id, _)| id.clone())
            {
                values.remove(&oldest);
            }
        }
        values.insert(
            id.clone(),
            Entry {
                plugin_id: plugin_id.into(),
                session_id: session_id.into(),
                name: name.into(),
                created: Instant::now(),
            },
        );
    }
    id
}

/// Resolve a capability for the calling plugin. Resource ids from another
/// plugin, another session or an expired entry are rejected.
pub fn resolve(plugin_id: &str, session_id: Option<&str>, id: &str) -> Result<String, String> {
    let mut values = registry().lock().map_err(|_| "read registry unavailable")?;
    values.retain(|_, entry| entry.created.elapsed() < TTL);
    let entry = values.get(id).ok_or("read capability expired")?;
    if entry.plugin_id != plugin_id {
        return Err("read capability belongs to another plugin".into());
    }
    if let Some(session_id) = session_id {
        if entry.session_id != session_id {
            return Err("read capability belongs to another session".into());
        }
    }
    Ok(entry.name.clone())
}
