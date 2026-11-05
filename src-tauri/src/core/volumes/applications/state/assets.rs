use super::*;

pub fn asset_path(uri: &str) -> Result<std::path::PathBuf, String> {
    if let Some(target) = pending_target(uri) {
        if target.kind != PendingKind::Application {
            return Err("folder has no application asset".into());
        }
        return imported_asset_path(
            target
                .asset_id
                .as_deref()
                .ok_or("missing application asset ID")?,
            &target.extension,
        );
    }
    let app = source(uri)?;
    match app.source {
        ApplicationSource::Imported {
            asset_id,
            extension,
        } => imported_asset_path(&asset_id, &extension),
        ApplicationSource::System { .. } => Err("system application has no private asset".into()),
    }
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

pub(crate) fn applications_root_path() -> Result<std::path::PathBuf, String> {
    let fiex = dirs::home_dir()
        .ok_or("cannot locate home directory")?
        .join(".fiex");
    std::fs::create_dir_all(&fiex).map_err(|error| error.to_string())?;
    let fiex = dunce::canonicalize(fiex).map_err(|error| error.to_string())?;
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
    let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
    if root.parent() != Some(fiex.as_path()) {
        return Err("Applications asset root escaped its private directory".into());
    }
    Ok(root)
}
