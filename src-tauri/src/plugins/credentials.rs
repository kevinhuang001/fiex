//! Credential backend selection, migration and cached OS vault access.
use super::storage::validate;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
fn fallback_file() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .ok_or_else(|| "application data directory unavailable".to_string())
        .map(|data| data.join("fiex").join("secrets.json"))
}

fn fallback_key(plugin: &str, scope: &str, key: &str) -> String {
    format!("{plugin}:{scope}:{key}")
}

fn load_fallback(path: &Path) -> Result<BTreeMap<String, String>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("secret store unreadable: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(error.to_string()),
    }
}

/// Atomic 0600 replacement: secrets never land in the registry file and a
/// crash leaves either the old or the new store, never a partial one.
fn save_fallback(path: &Path, items: &BTreeMap<String, String>) -> Result<(), String> {
    let parent = path.parent().ok_or("invalid secret storage path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(items).map_err(|error| error.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

/// The whole vault uses a single OS entry. Reads (including failures) are
/// cached for the process, so listing connections cannot trigger N prompts.
#[derive(Default)]
struct Credentials {
    backend: Option<String>,
    vault: Option<Result<BTreeMap<String, String>, String>>,
    legacy: BTreeMap<String, Result<Option<String>, String>>,
}
fn credentials() -> &'static Mutex<Credentials> {
    static STATE: OnceLock<Mutex<Credentials>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
fn backend_file() -> Result<PathBuf, String> {
    Ok(fallback_file()?.with_file_name("credentials-backend.json"))
}
impl Credentials {
    fn backend(&mut self) -> Result<&str, String> {
        if self.backend.is_none() {
            self.backend = Some(match fs::read(backend_file()?) {
                Ok(bytes) => serde_json::from_slice::<String>(&bytes).map_err(|e| e.to_string())?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => "local".into(),
                Err(e) => return Err(e.to_string()),
            });
        }
        let backend = self.backend.as_deref().unwrap();
        if !["local", "oskeyring"].contains(&backend) {
            return Err("invalid credential backend setting".into());
        }
        Ok(backend)
    }
    fn load(&mut self) -> Result<BTreeMap<String, String>, String> {
        if self.backend()? == "local" {
            return load_fallback(&fallback_file()?);
        }
        cached_vault(&mut self.vault, || match vault_entry()?.get_password() {
            Ok(value) => serde_json::from_str(&value).map_err(|e| e.to_string()),
            Err(keyring::v1::Error::NoEntry) => Ok(BTreeMap::new()),
            Err(e) => Err(format!("credential store access failed: {e}")),
        })
    }
    fn save(&mut self, items: BTreeMap<String, String>) -> Result<(), String> {
        if self.backend()? == "local" {
            return save_fallback(&fallback_file()?, &items);
        }
        if self
            .vault
            .as_ref()
            .is_some_and(|cached| cached.as_ref().is_ok_and(|old| old == &items))
        {
            return Ok(());
        }
        let result = vault_entry().and_then(|entry| {
            entry
                .set_password(&serde_json::to_string(&items).map_err(|e| e.to_string())?)
                .map_err(|e| format!("credential store write failed: {e}"))
        });
        if let Err(error) = result {
            self.vault = Some(Err(error.clone()));
            return Err(error);
        }
        self.vault = Some(Ok(items));
        Ok(())
    }
}
fn cached_vault(
    cache: &mut Option<Result<BTreeMap<String, String>, String>>,
    read: impl FnOnce() -> Result<BTreeMap<String, String>, String>,
) -> Result<BTreeMap<String, String>, String> {
    cache.get_or_insert_with(read).clone()
}
fn vault_entry() -> Result<keyring::v1::Entry, String> {
    keyring::v1::Entry::new("org.fiex.plugin", "credentials-vault")
        .map_err(|e| format!("system credential store unavailable: {e}"))
}
pub fn credential_backend() -> Result<String, String> {
    let mut state = credentials()
        .lock()
        .map_err(|_| "credential store locked")?;
    Ok(state.backend()?.to_string())
}
/// Explicit setting change migrates the complete vault before committing the
/// preference. Refusals leave the source intact and never silently downgrade.
pub fn set_credential_backend(backend: &str) -> Result<(), String> {
    if !["local", "oskeyring"].contains(&backend) {
        return Err("invalid credential backend".into());
    }
    let mut state = credentials()
        .lock()
        .map_err(|_| "credential store locked")?;
    if state.backend()? == backend {
        return Ok(());
    }
    let source = state.load()?;
    let previous = state.backend.clone();
    state.backend = Some(backend.into());
    // Switching is an explicit retry after a previously denied keyring read.
    state.vault = None;
    let result = (|| {
        state.save(source)?;
        let path = backend_file()?;
        let parent = path.parent().ok_or("invalid backend path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        temp.write_all(&serde_json::to_vec(backend).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist(&path).map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    })();
    if let Err(error) = result {
        state.backend = previous;
        state.vault = None;
        return Err(error);
    }
    // The selected destination is now durable. Remove the inactive local copy.
    if backend == "oskeyring" {
        save_fallback(&fallback_file()?, &BTreeMap::new())?;
    }
    Ok(())
}
pub fn secret_set(plugin: &str, scope: &str, key: &str, value: &str) -> Result<(), String> {
    validate(plugin, scope, key)?;
    if value.len() > 65536 {
        return Err("secret too large".into());
    }
    let mut state = credentials()
        .lock()
        .map_err(|_| "credential store locked")?;
    let mut items = state.load()?;
    let id = fallback_key(plugin, scope, key);
    items.remove(&format!("!deleted:{id}"));
    items.insert(id, value.into());
    state.save(items)
}
/// Called only by authenticated host IPC; never expose values to plugin frames.
pub fn secret_get(plugin: &str, scope: &str, key: &str) -> Result<String, String> {
    validate(plugin, scope, key)?;
    let mut state = credentials()
        .lock()
        .map_err(|_| "credential store locked")?;
    let id = fallback_key(plugin, scope, key);
    let mut items = state.load()?;
    if let Some(value) = items.get(&id) {
        return Ok(value.clone());
    }
    // Old versions stored one keyring entry per credential. Read each at most
    // once, and only after the user explicitly selected OS storage. A durable
    // tombstone prevents deleted legacy credentials from being resurrected.
    if state.backend()? == "oskeyring" && !items.contains_key(&format!("!deleted:{id}")) {
        let legacy = state
            .legacy
            .entry(id.clone())
            .or_insert_with(|| {
                let entry =
                    keyring::v1::Entry::new("org.fiex.plugin", &id).map_err(|e| e.to_string())?;
                match entry.get_password() {
                    Ok(value) => Ok(Some(value)),
                    Err(keyring::v1::Error::NoEntry) => Ok(None),
                    Err(e) => Err(format!("credential store access failed: {e}")),
                }
            })
            .clone()?;
        if let Some(value) = legacy {
            items.insert(id, value.clone());
            state.save(items)?;
            return Ok(value);
        }
    }
    Err("no matching credential".into())
}
pub fn secret_delete(plugin: &str, scope: &str, key: &str) -> Result<(), String> {
    validate(plugin, scope, key)?;
    let mut state = credentials()
        .lock()
        .map_err(|_| "credential store locked")?;
    let mut items = state.load()?;
    let id = fallback_key(plugin, scope, key);
    items.remove(&id);
    items.insert(format!("!deleted:{id}"), String::new());
    state.legacy.remove(&id);
    state.save(items)
}
pub fn secret_available() -> bool {
    credentials()
        .lock()
        .is_ok_and(|mut state| state.load().is_ok())
}

/// Typed wire status for the authenticated SDK keychain service. Access denied
/// includes a locked store or a platform/user authorization refusal; it is not
/// conflated with an absent credential or an unavailable backend.
pub fn secret_response(
    plugin: &str,
    scope: &str,
    key: &str,
    operation: &str,
    value: Option<&str>,
) -> Result<Value, String> {
    use serde_json::json;
    fn failure(message: String) -> Value {
        let status = if message.contains("access failed") || message.contains("denied") {
            "denied"
        } else {
            "unavailable"
        };
        json!({"status":status,"message":message})
    }
    validate(plugin, scope, key)?;
    Ok(match operation {
        "get" => match secret_get(plugin, scope, key) {
            Ok(value) => json!({"status":"present","value":value}),
            Err(error) if error == "no matching credential" => json!({"status":"missing"}),
            Err(error) => failure(error),
        },
        "set" => {
            let value = value.ok_or("missing secret value")?;
            if value.len() > 65536 {
                return Err("secret too large".into());
            }
            match secret_set(plugin, scope, key, value) {
                Ok(()) => json!({"status":"ok"}),
                Err(error) => failure(error),
            }
        }
        "delete" => match secret_delete(plugin, scope, key) {
            Ok(()) => json!({"status":"ok"}),
            Err(error) => failure(error),
        },
        _ => return Err("unknown credential operation".into()),
    })
}

#[cfg(test)]
mod tests;
