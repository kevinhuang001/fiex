//! Private storage paths for imported application assets.

use crate::{valid_extension, valid_id};

#[cfg(feature = "desktop")]
fn home_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir()
}

#[cfg(not(feature = "desktop"))]
fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

pub fn applications_root_path() -> Result<std::path::PathBuf, String> {
    let home = home_dir().ok_or("cannot locate home directory")?;
    let fiex = home.join(".fiex");
    std::fs::create_dir_all(&fiex).map_err(|error| error.to_string())?;
    let fiex = std::fs::canonicalize(fiex).map_err(|error| error.to_string())?;
    let root = fiex.join("applications");
    match std::fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("Applications asset root must be a private directory".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&root).map_err(|error| error.to_string())?;
        }
        Err(error) => return Err(error.to_string()),
    }
    let root = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    if root.parent() != Some(fiex.as_path()) {
        return Err("Applications asset root escaped its private directory".into());
    }
    Ok(root)
}

pub fn imported_asset_path(asset_id: &str, extension: &str) -> Result<std::path::PathBuf, String> {
    if !valid_id(asset_id) || !valid_extension(extension) {
        return Err("invalid application asset ID".into());
    }
    let root = applications_root_path()?;
    let name = if extension.is_empty() {
        asset_id.to_owned()
    } else {
        format!("{asset_id}.{extension}")
    };
    Ok(root.join(name))
}
