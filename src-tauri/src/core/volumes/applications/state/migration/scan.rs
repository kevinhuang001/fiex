//! Scans legacy Applications entries and constructs virtual folder nodes.
use super::*;
use std::{
    collections::HashMap,
    path::{Component, Path},
};

pub(super) fn fallback_system_name(locator: &str) -> String {
    let path = locator
        .strip_prefix("path:")
        .or_else(|| locator.strip_prefix("aumid:"))
        .unwrap_or(locator)
        .replace('\\', "/");
    let candidate = if let Some((_, app_id)) = path.rsplit_once('!') {
        app_id
    } else {
        Path::new(&path)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or(&path)
    };
    let safe = candidate
        .chars()
        .filter(|character| !matches!(character, '/' | '\\' | ':' | '\0'))
        .collect::<String>()
        .trim()
        .to_owned();
    if safe.is_empty() {
        "Unavailable application".into()
    } else {
        safe
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn scan_directory(
    root: &Path,
    relative_dir: &Path,
    parent_id: Option<String>,
    state: &mut ApplicationsState,
    folder_by_relative: &mut HashMap<String, Option<String>>,
    assets: &mut Vec<AssetMove>,
    recovery: &mut Vec<String>,
    directories: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let directory = root.join(relative_dir);
    let mut children = std::fs::read_dir(&directory)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        let relative = relative_dir.join(child.file_name());
        let relative_text = relative
            .to_str()
            .ok_or("legacy Applications contains a non-UTF8 path")?
            .replace('\\', "/");
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            recovery.push(relative_text);
            continue;
        }
        if metadata.is_dir()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
            && !is_application(&path, true)
        {
            recovery.push(relative_text);
        } else if metadata.is_dir() && !is_application(&path, true) {
            let id = ensure_folder_path(&relative_text, state, folder_by_relative)?;
            directories.push(relative_text);
            scan_directory(
                root,
                &relative,
                Some(id),
                state,
                folder_by_relative,
                assets,
                recovery,
                directories,
                warnings,
            )?;
        } else if is_application(&path, metadata.is_dir()) {
            let id = super::super::super::random_id()?;
            let asset_id = super::super::super::random_id()?;
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or("legacy application has an invalid name")?
                .to_owned();
            let order = super::super::super::next_order(state, parent_id.as_deref());
            state.apps.insert(
                id.clone(),
                ApplicationNode {
                    id,
                    parent: parent_id.clone(),
                    display_name: name,
                    source: ApplicationSource::Imported {
                        asset_id: asset_id.clone(),
                        extension: extension.clone(),
                    },
                    order,
                    unavailable: false,
                },
            );
            assets.push(AssetMove {
                source: relative_text,
                asset_id,
                extension,
            });
        } else if metadata.is_file() {
            recovery.push(relative_text);
        } else {
            warnings.push(format!("unsupported legacy node retained: {relative_text}"));
        }
    }
    Ok(())
}

fn is_application(path: &Path, is_dir: bool) -> bool {
    if is_dir
        && !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
    {
        return false;
    }
    let target = crate::core::files::entry(path)
        .ok()
        .and_then(|entry| entry.link_target);
    crate::core::volumes::applications::classify_host_application(
        &path.to_string_lossy(),
        target.as_deref(),
    )
}

pub(super) fn ensure_folder_path(
    relative: &str,
    state: &mut ApplicationsState,
    folder_by_relative: &mut HashMap<String, Option<String>>,
) -> Result<String, String> {
    let path = super::super::safe_relative(relative)?;
    let mut parent_rel = String::new();
    let mut parent_id = None;
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err("invalid Applications folder component".into());
        };
        let name = component
            .to_str()
            .ok_or("legacy Applications contains a non-UTF8 folder name")?;
        parent_rel = if parent_rel.is_empty() {
            name.to_owned()
        } else {
            format!("{parent_rel}/{name}")
        };
        if let Some(existing) = folder_by_relative.get(&parent_rel) {
            parent_id = existing.clone();
            continue;
        }
        let id = super::super::super::random_id()?;
        let order = super::super::super::next_order(state, parent_id.as_deref());
        state.folders.insert(
            id.clone(),
            VirtualFolder {
                id: id.clone(),
                parent: parent_id.clone(),
                name: name.to_owned(),
                order,
            },
        );
        folder_by_relative.insert(parent_rel.clone(), Some(id.clone()));
        parent_id = Some(id);
    }
    parent_id.ok_or_else(|| "legacy Applications folder has no components".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_windows_apps_keep_a_displayable_name() {
        assert_eq!(
            fallback_system_name("aumid:Vendor.Package!Editor"),
            "Editor"
        );
        assert_eq!(fallback_system_name("path:C:\\Apps\\Editor.exe"), "Editor");
    }
}
