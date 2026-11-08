//! Native file association queries and explicit application launches.

use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWithCandidate {
    /// Opaque to UI callers; the owning platform service resolves this ID.
    pub id: String,
    pub name: String,
    pub icon_source: Option<String>,
    pub is_default: bool,
    pub is_recommended: bool,
}

#[cfg(windows)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsApplication {
    pub name: String,
    pub aumid: String,
    pub path: String,
    pub parsing_name: String,
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub fn open_with_candidates(
    file: &Path,
    extension: &str,
    mime_type: Option<&str>,
) -> Result<Vec<OpenWithCandidate>, String> {
    #[cfg(windows)]
    {
        let _ = (file, mime_type);
        windows::candidates(extension)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (extension, mime_type);
        macos::candidates(file)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = extension;
        linux::candidates(file, mime_type)
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = (file, extension, mime_type);
        Ok(Vec::new())
    }
}

pub fn open_with_applications(file: &Path) -> Result<Vec<OpenWithCandidate>, String> {
    #[cfg(windows)]
    {
        let _ = file;
        windows::applications()
    }
    #[cfg(target_os = "macos")]
    {
        let _ = file;
        macos::applications()
    }
    #[cfg(target_os = "linux")]
    {
        let _ = file;
        linux::applications()
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = file;
        Ok(Vec::new())
    }
}

pub fn open_with_application(file: &Path, application_id: &str) -> Result<(), String> {
    if !file.is_file() {
        return Err("The selected file no longer exists".into());
    }
    #[cfg(windows)]
    {
        windows::open_application(file, application_id)
    }
    #[cfg(target_os = "macos")]
    {
        macos::open_application(file, application_id)
    }
    #[cfg(target_os = "linux")]
    {
        linux::open_application(file, application_id)
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = (file, application_id);
        Err("Open With is unavailable on this platform".into())
    }
}

pub fn open_with(
    file: &Path,
    extension: &str,
    mime_type: Option<&str>,
    application_id: &str,
) -> Result<(), String> {
    if !file.is_file() {
        return Err("The selected file no longer exists".into());
    }
    #[cfg(windows)]
    {
        let _ = mime_type;
        windows::open(file, extension, application_id)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (extension, mime_type);
        macos::open(file, application_id)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = extension;
        linux::open(file, mime_type.unwrap_or_default(), application_id)
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = (file, extension, application_id);
        Err("Open With is unavailable on this platform".into())
    }
}

#[cfg(windows)]
pub fn enumerate_windows_applications() -> Result<Vec<WindowsApplication>, String> {
    windows::apps_folder().map(|items| {
        items
            .into_iter()
            .map(|item| WindowsApplication {
                name: item.name,
                aumid: item.aumid,
                path: item.path,
                parsing_name: item.parsing_name,
            })
            .collect()
    })
}
