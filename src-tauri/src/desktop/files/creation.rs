use super::*;

#[tauri::command]
pub(crate) async fn create_entry(
    path: String,
    name: String,
    state: tauri::State<'_, Operations>,
) -> Result<String, String> {
    if path == crate::core::volumes::applications::URI
        || crate::core::volumes::applications::is_user(&path)
    {
        return crate::core::volumes::applications::create_stored(&path, &name, true);
    }
    if !crate::plugins::vfs::handles(&path) {
        crate::plugins::mounts::reject_local(&path)?;
    }
    // Folder creation follows the owning adapter mkdir capability.
    crate::plugins::resource::file_capabilities::require(
        &path,
        crate::plugins::provider_registry::FsOperation::Mkdir,
    )?;
    if let Some(route) = crate::usbfs::route(&path) {
        crate::usbfs::create(&route, &name, true)?;
        return Ok(format!("{}/{}", path.trim_end_matches('/'), name));
    }
    if remote::is_remote(&path) {
        return remote::create(&path, &name).await;
    }
    if crate::plugins::vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::plugins::vfs::create(&path, &name)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    state.create(&path, &name, true)
}

#[tauri::command]
pub(crate) async fn rename_entry(
    path: String,
    name: String,
    state: tauri::State<'_, Operations>,
) -> Result<String, String> {
    if crate::core::volumes::applications::is_user(&path) {
        return crate::core::volumes::applications::rename_stored(&path, &name);
    }
    if !crate::plugins::vfs::handles(&path) {
        crate::plugins::mounts::reject_local(&path)?;
    }
    crate::plugins::resource::file_capabilities::require(
        &path,
        crate::plugins::provider_registry::FsOperation::Rename,
    )?;
    if let Some(route) = crate::usbfs::route(&path) {
        crate::usbfs::rename(&route, &name)?;
        return Ok(format!(
            "{}/{}",
            path.rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("/"),
            name
        ));
    }
    if remote::is_remote(&path) {
        return remote::rename(&path, &name).await;
    }
    if crate::plugins::vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::plugins::vfs::rename(&path, &name)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    state.rename(&path, &name)
}
/// Creating a child file requires the parent's create capability (Mkdir),
/// then a bounded write stream on that child. Directory nodes do not advertise
/// ordinary file-content Write, and must never need it to add children.
#[tauri::command]
pub(crate) async fn create_file_entry(path: String, name: String) -> Result<String, String> {
    if path == crate::core::volumes::applications::URI
        || crate::core::volumes::applications::is_user(&path)
    {
        return crate::core::volumes::applications::create_stored(&path, &name, false);
    }
    if !crate::plugins::vfs::handles(&path) {
        crate::plugins::mounts::reject_local(&path)?;
    }
    crate::plugins::mounts::segment(&name)?;
    crate::plugins::resource::file_capabilities::require(
        &path,
        crate::plugins::provider_registry::FsOperation::Mkdir,
    )?;
    if let Some(route) = crate::usbfs::route(&path) {
        crate::usbfs::create(&route, &name, false)?;
        return Ok(format!("{}/{}", path.trim_end_matches('/'), name));
    }
    if remote::is_remote(&path) {
        return remote::create_file(&path, &name).await;
    }
    if crate::plugins::vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::plugins::vfs::create_file(&path, &name)
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    let target = std::path::Path::new(&path).join(&name);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|error| error.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}
