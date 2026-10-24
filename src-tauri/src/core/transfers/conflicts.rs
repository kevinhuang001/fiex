//! Conflict detection: describe both sides of a collision and apply the
//! decision the user made earlier (if any).

use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::UNIX_EPOCH};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVersion {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Conflict {
    pub key: String,
    pub source: FileVersion,
    pub destination: FileVersion,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Replace,
    Skip,
    KeepBoth,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Resolution {
    pub key: String,
    pub action: Action,
}

/// What the plan should do with one destination that already exists.
pub(super) enum Decision {
    /// No choice recorded for this exact pair yet; the caller collects the
    /// conflict and asks the user.
    Pending(Conflict),
    Skip,
    KeepBoth,
    Replace,
}

/// Describe both sides of a collision and match it against earlier choices.
/// A stale choice (the files changed since the prompt) is reported as a fresh
/// conflict because the key covers timestamps and sizes.
pub(super) fn decide(
    src: &Path,
    dest: &Path,
    resolutions: &[Resolution],
    keep_both: bool,
) -> Result<Decision, String> {
    let (source, source_stamp) = version(src)?;
    let (destination, dest_stamp) = version(dest)?;
    let key = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        source.path, destination.path, source_stamp, dest_stamp, source.size, destination.size
    );
    let action = if keep_both {
        Some(Action::KeepBoth)
    } else {
        resolutions.iter().find(|r| r.key == key).map(|r| r.action)
    };
    Ok(match action {
        None => Decision::Pending(Conflict {
            key,
            source,
            destination,
        }),
        Some(Action::Skip) => Decision::Skip,
        Some(Action::KeepBoth) => Decision::KeepBoth,
        Some(Action::Replace) => Decision::Replace,
    })
}

fn version(path: &Path) -> Result<(FileVersion, u128), String> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let stamp = m
        .modified()
        .ok()
        .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    Ok((
        FileVersion {
            path: path.to_string_lossy().into_owned(),
            size: m.len(),
            modified: stamp.as_secs(),
            is_dir: m.is_dir(),
        },
        stamp.as_nanos(),
    ))
}
