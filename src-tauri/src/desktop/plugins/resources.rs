use super::*;

/// Host-owned native file chooser. Returns only a selected path; plugin
/// code never gets a handle to the chooser UI or a directory browsing grant.
#[tauri::command]
pub(crate) fn plugin_file_dialog(
    mode: String,
    directory: Option<String>,
    suggested_name: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<String>, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path) = directory.filter(|v| std::path::Path::new(v).is_dir()) {
        dialog = dialog.set_directory(path);
    }
    if let Some(name) = suggested_name {
        let path = std::path::Path::new(&name);
        if path.file_name().is_none()
            || path.components().count() != 1
            || name.contains(['\\', '/', '\0'])
        {
            return Err("invalid suggested filename".into());
        }
        dialog = dialog.set_file_name(name);
    }
    if let Some(exts) = extensions {
        if exts.len() > 20
            || exts.iter().any(|v| {
                v.is_empty() || v.len() > 16 || !v.chars().all(|c| c.is_ascii_alphanumeric())
            })
        {
            return Err("invalid file extension filter".into());
        }
        if !exts.is_empty() {
            let refs = exts.iter().map(String::as_str).collect::<Vec<_>>();
            dialog = dialog.add_filter(crate::i18n::t("文件类型"), &refs);
        }
    }
    let selection = match mode.as_str() {
        "openFile" => dialog.pick_file(),
        "openDirectory" => dialog.pick_folder(),
        "saveFile" => dialog.save_file(),
        _ => return Err("unsupported file picker mode".into()),
    };
    Ok(selection.map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
pub(crate) fn plugin_files_dialog(
    directory: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path) = directory.filter(|v| std::path::Path::new(v).is_dir()) {
        dialog = dialog.set_directory(path);
    }
    if let Some(exts) = extensions {
        if exts.len() > 20
            || exts.iter().any(|v| {
                v.is_empty() || v.len() > 16 || !v.chars().all(|c| c.is_ascii_alphanumeric())
            })
        {
            return Err("invalid extension filter".into());
        }
        if !exts.is_empty() {
            let refs = exts.iter().map(String::as_str).collect::<Vec<_>>();
            dialog = dialog.add_filter(crate::i18n::t("文件类型"), &refs);
        }
    }
    Ok(dialog.pick_files().map(|paths| {
        paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }))
}
/// One window reports all live history references and explicitly saved paths.
#[tauri::command]
pub(crate) fn vfs_retain_views(
    window: tauri::Window,
    paths: Vec<String>,
    durable: Vec<String>,
) -> Result<(), String> {
    crate::plugins::mounts::retain(window.label(), &paths, &durable)
}

/// Only the owner of an existing view may recover its already authorized
/// resource context. Other plugins see the URI, never the backing local root.
#[tauri::command]
pub(crate) fn plugin_context_directory(plugin_id: String, path: String) -> Result<String, String> {
    if !crate::plugins::mounts::handles(&path) {
        return Ok(path);
    }
    let (_, source) = crate::plugins::mounts::resolve(&path)?;
    match source {
        crate::plugins::mounts::Source::View(view) if view.descriptor.plugin_id == plugin_id => {
            Ok(view.descriptor.resource_root)
        }
        _ => Ok(path),
    }
}
