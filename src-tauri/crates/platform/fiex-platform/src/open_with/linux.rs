use super::OpenWithCandidate;
use gio::prelude::*;
use std::path::Path;

pub(super) fn candidates(
    _file: &Path,
    mime_type: Option<&str>,
) -> Result<Vec<OpenWithCandidate>, String> {
    let Some(mime_type) = mime_type.filter(|value| !value.is_empty()) else {
        return Ok(Vec::new());
    };
    let content_type = gio::content_type_from_mime_type(mime_type)
        .map(|value| value.to_string())
        .unwrap_or_else(|| mime_type.to_owned());
    let default_id = gio::AppInfo::default_for_type(&content_type, false)
        .and_then(|app| app.id().map(|id| id.to_string()));
    let recommended = gio::AppInfo::recommended_for_type(&content_type);
    let recommended_ids = recommended
        .iter()
        .filter_map(|app| app.id().map(|id| id.to_string()))
        .collect::<std::collections::HashSet<_>>();
    let mut seen = std::collections::HashSet::new();
    let mut candidates = gio::AppInfo::all_for_type(&content_type)
        .into_iter()
        .filter(|app| app.should_show() && app.supports_files())
        .filter_map(|app| {
            let id = app.id()?.to_string();
            if !seen.insert(id.clone()) {
                return None;
            }
            Some(OpenWithCandidate {
                is_default: default_id.as_deref() == Some(&id),
                is_recommended: recommended_ids.contains(&id),
                icon_source: app
                    .icon()
                    .and_then(|icon| icon.to_string().map(|s| s.to_string())),
                id,
                name: app.name().to_string(),
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| right.is_recommended.cmp(&left.is_recommended))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(candidates)
}

pub(super) fn open(file: &Path, mime_type: &str, application_id: &str) -> Result<(), String> {
    let content_type = gio::content_type_from_mime_type(mime_type)
        .map(|value| value.to_string())
        .unwrap_or_else(|| mime_type.to_owned());
    let app = gio::AppInfo::all_for_type(&content_type)
        .into_iter()
        .find(|app| app.id().is_some_and(|id| id == application_id))
        .ok_or("The selected application is no longer associated with this file type")?;
    app.launch(&[gio::File::for_path(file)], None::<&gio::AppLaunchContext>)
        .map_err(|error| error.to_string())
}

pub(super) fn applications() -> Result<Vec<OpenWithCandidate>, String> {
    let mut seen = std::collections::HashSet::new();
    let mut output = gio::AppInfo::all()
        .into_iter()
        .filter(|app| app.should_show() && app.supports_files())
        .filter_map(|app| {
            let id = app.id()?.to_string();
            if !seen.insert(id.clone()) {
                return None;
            }
            Some(OpenWithCandidate {
                icon_source: app
                    .icon()
                    .and_then(|icon| icon.to_string().map(|s| s.to_string())),
                id,
                name: app.name().to_string(),
                is_default: false,
                is_recommended: false,
            })
        })
        .collect::<Vec<_>>();
    output.sort_by_key(|candidate| candidate.name.to_lowercase());
    Ok(output)
}

pub(super) fn open_application(file: &Path, application_id: &str) -> Result<(), String> {
    let app = gio::AppInfo::all()
        .into_iter()
        .find(|app| {
            app.should_show() && app.supports_files() && app.id().as_deref() == Some(application_id)
        })
        .ok_or("The selected application is no longer installed")?;
    app.launch(&[gio::File::for_path(file)], None::<&gio::AppLaunchContext>)
        .map_err(|error| error.to_string())
}
