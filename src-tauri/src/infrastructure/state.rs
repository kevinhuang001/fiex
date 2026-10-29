//! Durable user intent, independent of cache expiration and eviction.
//! Namespace updates are serialized and atomically replaced on disk.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs, io::Write, path::PathBuf,
    sync::{Mutex, OnceLock},
};

const SCHEMA: u32 = 1;
const MAX_STATE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Default, Serialize, Deserialize)]
struct State {
    schema: u32,
    revision: u64,
    values: BTreeMap<String, Value>,
}
static STORE: OnceLock<Mutex<State>> = OnceLock::new();

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("fiex").join("state.json"))
}
fn store() -> &'static Mutex<State> {
    STORE.get_or_init(|| {
        let data = path().and_then(|path| {
            let bytes = fs::read(path).ok()?;
            if bytes.len() > MAX_STATE_BYTES { return None; }
            serde_json::from_slice::<State>(&bytes).ok().filter(|s| s.schema == SCHEMA)
        }).unwrap_or_else(|| State { schema: SCHEMA, ..State::default() });
        Mutex::new(data)
    })
}
fn valid_component(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 &&
        value.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn validate(namespace: &str, key: &str) -> Result<String, String> {
    if !matches!(namespace, "explorer" | "picker" | "search" | "layout" | "providers")
        || !valid_component(key) {
        return Err("invalid state namespace/key".into());
    }
    Ok(format!("{namespace}.{key}"))
}
pub fn get(namespace: &str, key: &str) -> Result<Option<Value>, String> {
    let name = validate(namespace, key)?;
    Ok(store().lock().map_err(|_| "state unavailable")?.values.get(&name).cloned())
}
pub fn put(namespace: &str, key: &str, value: Value) -> Result<(), String> {
    let name = validate(namespace, key)?;
    let mut state = store().lock().map_err(|_| "state unavailable")?;
    let previous = state.values.insert(name.clone(), value);
    state.revision = state.revision.saturating_add(1);
    let result = persist(&state);
    if result.is_err() {
        match previous {
            Some(value) => { state.values.insert(name, value); },
            None => { state.values.remove(&name); },
        }
        state.revision = state.revision.saturating_sub(1);
    }
    result
}
pub fn remove(namespace: &str, key: &str) -> Result<(), String> {
    let name = validate(namespace, key)?;
    let mut state = store().lock().map_err(|_| "state unavailable")?;
    if let Some(previous) = state.values.remove(&name) {
        state.revision = state.revision.saturating_add(1);
        if let Err(error) = persist(&state) {
            state.revision = state.revision.saturating_sub(1);
            state.values.insert(name, previous);
            return Err(error);
        }
    }
    Ok(())
}
fn persist(state: &State) -> Result<(), String> {
    let bytes = serde_json::to_vec(state).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_STATE_BYTES { return Err("state file exceeds size limit".into()); }
    let path = path().ok_or("no application configuration directory")?;
    let dir = path.parent().ok_or("invalid state directory")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let mut temp = tempfile::Builder::new().prefix(".state-")
        .tempfile_in(dir).map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.flush().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
