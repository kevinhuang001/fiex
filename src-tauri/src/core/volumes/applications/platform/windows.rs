use super::super::{decode, encode, entry, is_executable_target, URI};
use crate::core::files::Entry;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
pub(in crate::core::volumes::applications) fn resolve_windows_executable(
    value: &str,
) -> Option<String> {
    if !is_executable_target(value) {
        return None;
    }
    let path = if value.starts_with('{') {
        crate::core::volumes::windows_shell::resolve_known_folder_path(value)?
    } else {
        PathBuf::from(value)
    };
    path.is_file().then(|| path.to_string_lossy().into_owned())
}

#[cfg(target_os = "windows")]
fn looks_like_path(value: &str) -> bool {
    value.len() > 2
        && (value.as_bytes()[1] == b':' || value.starts_with("\\\\") || value.starts_with('{'))
}

#[cfg(target_os = "windows")]
pub(in crate::core::volumes::applications) fn platform_list() -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for app in fiex_platform::enumerate_windows_applications()? {
        if app.name.trim().is_empty() {
            continue;
        }
        let aumid = if app.aumid.is_empty() {
            app.parsing_name
                .strip_prefix("shell:AppsFolder\\")
                .unwrap_or_default()
                .to_owned()
        } else {
            app.aumid
        };
        let path_is_file = looks_like_path(&app.path);
        let resolved_path = if path_is_file {
            resolve_windows_executable(&app.path)
        } else {
            None
        };
        let id = if looks_like_path(&aumid) {
            let Some(target) = resolve_windows_executable(&aumid).or_else(|| resolved_path.clone())
            else {
                continue;
            };
            format!("path:{target}")
        } else if !aumid.is_empty() {
            if !aumid.contains('!') && path_is_file && resolved_path.is_none() {
                continue;
            }
            format!("aumid:{aumid}")
        } else if let Some(target) = resolved_path {
            format!("path:{target}")
        } else {
            continue;
        };

        let mut item = entry(app.name, format!("{URI}{}", encode(&id)), "app", 0);
        if let Some(aumid) = id.strip_prefix("aumid:") {
            item.link_target = Some(format!("shell:AppsFolder\\{aumid}"));
        } else if let Some(target) = id.strip_prefix("path:") {
            item.link_target = Some(target.to_owned());
        }
        entries.push(item);
    }
    entries.sort_by_key(|left| left.name.to_lowercase());
    Ok(entries)
}

#[cfg(target_os = "windows")]
pub(in crate::core::volumes::applications) fn platform_open(encoded: &str) -> Result<(), String> {
    let id = decode(encoded)?;
    let target = if let Some(aumid) = id.strip_prefix("aumid:") {
        format!("shell:AppsFolder\\{aumid}")
    } else if let Some(path) = id.strip_prefix("path:") {
        resolve_windows_executable(path)
            .ok_or_else(|| crate::i18n::t("无法启动应用").to_string())?
    } else {
        return Err(crate::i18n::t("无法启动应用").into());
    };
    open::that_detached(target).map_err(|error| error.to_string())
}
