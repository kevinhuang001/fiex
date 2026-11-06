//! Storage owner validation and stable on-disk path mapping.
use super::super::owner::OwnerId;
use super::MAX_VALUE_BYTES;
use std::path::PathBuf;

pub(super) fn max_value_bytes(owner: &OwnerId, scope: &str, key: &str) -> usize {
    match owner {
        OwnerId::Core(core) => core.max_value_bytes(scope, key, MAX_VALUE_BYTES),
        OwnerId::Plugin(_) => MAX_VALUE_BYTES,
    }
}

pub(super) fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && value != "."
        && value != ".."
        && !value.starts_with('.')
}

pub fn validate(owner: &OwnerId, scope: &str, key: &str) -> Result<(), String> {
    if !owner.is_valid() || !valid_segment(scope) || !valid_segment(key) {
        return Err("invalid storage owner, scope or key".into());
    }
    Ok(())
}

fn base() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .ok_or("application data directory unavailable".to_string())
        .map(|d| d.join("fiex").join("plugin-state"))
}

pub(super) fn file(owner: &OwnerId, scope: &str) -> Result<PathBuf, String> {
    if !owner.is_valid() || !valid_segment(scope) {
        return Err("invalid storage owner or scope".into());
    }
    let directory = match owner {
        OwnerId::Plugin(plugin) => base()?.join(plugin),
        OwnerId::Core(key) => base()?.join("core").join(key.as_str()),
    };
    Ok(directory.join(format!("{scope}.json")))
}
