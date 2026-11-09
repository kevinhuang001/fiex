use super::OpenWithCandidate;
use objc2::rc::{Retained, autoreleasepool};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSString, NSURL};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn candidates(file: &Path) -> Result<Vec<OpenWithCandidate>, String> {
    autoreleasepool(|_| {
        let file_url = file_url(file)?;
        let workspace = NSWorkspace::sharedWorkspace();
        let default_id = workspace
            .URLForApplicationToOpenURL(&file_url)
            .and_then(|url| url.path())
            .map(|path| path.to_string());
        let applications = workspace.URLsForApplicationsToOpenURL(&file_url);
        let mut values = Vec::new();
        for index in 0..applications.count() {
            let Some(path) = applications.objectAtIndex(index).path() else {
                continue;
            };
            let id = path.to_string();
            if id.is_empty()
                || values
                    .iter()
                    .any(|value: &OpenWithCandidate| value.id == id)
            {
                continue;
            }
            values.push(OpenWithCandidate {
                is_default: default_id.as_deref() == Some(&id),
                is_recommended: true,
                icon_source: Some(id.clone()),
                id,
                name: applications
                    .objectAtIndex(index)
                    .lastPathComponent()
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| "Application".into()),
            });
        }
        values.sort_by(|left, right| {
            right
                .is_default
                .cmp(&left.is_default)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(values)
    })
}

pub(super) fn open(file: &Path, application_id: &str) -> Result<(), String> {
    let available = candidates(file)?;
    if !available
        .iter()
        .any(|candidate| candidate.id == application_id)
    {
        return Err("The selected application is no longer associated with this file type".into());
    }
    Command::new("open")
        .args(["-a", application_id, "--"])
        .arg(file)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(super) fn applications() -> Result<Vec<OpenWithCandidate>, String> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
    ];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut output = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("app")
                || !path.join("Contents/Info.plist").is_file()
            {
                continue;
            }
            let id = path.to_string_lossy().into_owned();
            if !seen.insert(id.clone()) {
                continue;
            }
            output.push(OpenWithCandidate {
                name: path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Application")
                    .to_owned(),
                icon_source: Some(id.clone()),
                id,
                is_default: false,
                is_recommended: false,
            });
        }
    }
    output.sort_by_key(|candidate| candidate.name.to_lowercase());
    Ok(output)
}

pub(super) fn open_application(file: &Path, application_id: &str) -> Result<(), String> {
    if !applications()?
        .iter()
        .any(|candidate| candidate.id == application_id)
    {
        return Err("The selected application is no longer installed".into());
    }
    Command::new("open")
        .args(["-a", application_id, "--"])
        .arg(file)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn file_url(path: &Path) -> Result<Retained<NSURL>, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "The selected file path is not valid Unicode".to_owned())?;
    let path = NSString::from_str(path);
    Ok(NSURL::fileURLWithPath(&path))
}
