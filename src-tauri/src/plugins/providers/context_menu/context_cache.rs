//! Per-right-click resource snapshots shared by Core and plugin menu queries.
use super::builtin;
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

const MENU_CONTEXT_TTL: Duration = Duration::from_secs(10);
const MENU_CONTEXT_LIMIT: usize = 512;

struct MenuContextFlight {
    result: Mutex<Option<Result<builtin::Context, String>>>,
    ready: Condvar,
}

enum MenuContextEntry {
    Loading(Arc<MenuContextFlight>),
    Ready {
        created: Instant,
        context: Box<builtin::Context>,
    },
}

fn menu_contexts() -> &'static Mutex<HashMap<String, MenuContextEntry>> {
    static CONTEXTS: OnceLock<Mutex<HashMap<String, MenuContextEntry>>> = OnceLock::new();
    CONTEXTS.get_or_init(Default::default)
}

fn cache_key(
    session_id: Option<&str>,
    directory: &str,
    paths: &[String],
) -> Result<Option<String>, String> {
    let Some(session_id) = session_id.filter(|value| valid_session_id(value)) else {
        return Ok(None);
    };
    let selection = serde_json::to_string(paths).map_err(|error| error.to_string())?;
    Ok(Some(format!("{session_id}\0{directory}\0{selection}")))
}

fn valid_session_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// Core and plugin requests from one right-click share the same resource
/// snapshot, so local Stat and VFS metadata discovery run only once.
pub(super) fn menu_context(
    directory: &str,
    paths: &[String],
    session_id: Option<&str>,
) -> Result<builtin::Context, String> {
    let Some(key) = cache_key(session_id, directory, paths)? else {
        return builtin::Context::load_for_menu(directory, paths, false);
    };
    let (flight, load) = {
        let mut contexts = menu_contexts()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        contexts.retain(|_, entry| match entry {
            MenuContextEntry::Loading(_) => true,
            MenuContextEntry::Ready { created, .. } => created.elapsed() < MENU_CONTEXT_TTL,
        });
        if let Some(entry) = contexts.get(&key) {
            match entry {
                MenuContextEntry::Ready { context, .. } => return Ok(context.as_ref().clone()),
                MenuContextEntry::Loading(flight) => (Arc::clone(flight), false),
            }
        } else {
            if contexts.len() >= MENU_CONTEXT_LIMIT {
                return Err("too many active context-menu snapshots".into());
            }
            let flight = Arc::new(MenuContextFlight {
                result: Mutex::new(None),
                ready: Condvar::new(),
            });
            contexts.insert(key.clone(), MenuContextEntry::Loading(Arc::clone(&flight)));
            (flight, true)
        }
    };
    if load {
        let result = builtin::Context::load_for_menu(directory, paths, false);
        {
            let mut contexts = menu_contexts()
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Ok(context) = &result {
                contexts.insert(
                    key,
                    MenuContextEntry::Ready {
                        created: Instant::now(),
                        context: Box::new(context.clone()),
                    },
                );
            } else {
                contexts.remove(&key);
            }
        }
        *flight
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(result.clone());
        flight.ready.notify_all();
        result
    } else {
        let mut result = flight
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while result.is_none() {
            result = flight
                .ready
                .wait(result)
                .unwrap_or_else(|error| error.into_inner());
        }
        result.as_ref().expect("context flight completed").clone()
    }
}

#[cfg(test)]
mod tests {
    use super::cache_key;

    #[test]
    fn only_bounded_ascii_session_ids_are_cached() {
        assert_eq!(cache_key(None, "/tmp", &[]).unwrap(), None);
        assert_eq!(cache_key(Some(""), "/tmp", &[]).unwrap(), None);
        assert_eq!(cache_key(Some("bad/token"), "/tmp", &[]).unwrap(), None);
        assert_eq!(
            cache_key(Some(&"x".repeat(129)), "/tmp", &[]).unwrap(),
            None
        );
        assert_eq!(
            cache_key(Some("session-7"), "/tmp", &["a.txt".into()]).unwrap(),
            Some("session-7\0/tmp\0[\"a.txt\"]".into())
        );
    }
}
