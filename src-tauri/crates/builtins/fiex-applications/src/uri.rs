//! Canonical URI encoding and resolution for the Applications virtual tree.
use crate::{ApplicationsState, URI};

pub fn child_uri(parent_uri: &str, segment: &str) -> String {
    let segment = encode_segment(segment);
    if parent_uri == URI {
        format!("{parent_uri}{segment}")
    } else {
        format!("{}/{segment}", parent_uri.trim_end_matches('/'))
    }
}

fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            encoded.push(char::from(b"0123456789ABCDEF"[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

fn decoded_segments(uri: &str) -> Option<Vec<String>> {
    let relative = uri.strip_prefix(URI)?;
    if relative.is_empty() {
        return Some(Vec::new());
    }
    relative
        .split('/')
        .map(|segment| {
            if segment.is_empty() {
                return None;
            }
            let name = urlencoding::decode(segment).ok()?.into_owned();
            if name.is_empty()
                || matches!(name.as_str(), "." | "..")
                || name.contains('/')
                || name.contains('\\')
            {
                return None;
            }
            Some(name)
        })
        .collect()
}

fn canonical_segments(segments: &[String]) -> String {
    segments
        .iter()
        .map(|segment| encode_segment(segment))
        .collect::<Vec<_>>()
        .join("/")
}

fn resolve_folder_segments(state: &ApplicationsState, segments: &[String]) -> Option<String> {
    let mut parent: Option<String> = None;
    for name in segments {
        let mut matches = state
            .folders
            .values()
            .filter(|folder| folder.parent == parent && folder.name == *name);
        let folder = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        parent = Some(folder.id.clone());
    }
    parent
}

/// Resolve a folder by the same visible path segments used by the browser.
pub fn folder_id(state: &ApplicationsState, uri: &str) -> Option<String> {
    let segments = decoded_segments(uri)?;
    let id = resolve_folder_segments(state, &segments)?;
    (folder_uri_in(state, &id).as_deref() == Some(uri)).then_some(id)
}

/// Resolve an application by its parent-folder path and visible display name.
pub fn app_id(state: &ApplicationsState, uri: &str) -> Option<String> {
    let segments = decoded_segments(uri)?;
    let (name, parents) = segments.split_last()?;
    let parent = resolve_folder_segments(state, parents);
    let mut matches = state
        .apps
        .values()
        .filter(|app| app.parent == parent && app.display_name == *name);
    let app = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    (app_uri_in(state, &app.id).as_deref() == Some(uri)).then(|| app.id.clone())
}

pub fn folder_uri_in(state: &ApplicationsState, id: &str) -> Option<String> {
    let mut segments = Vec::new();
    let mut current = state.folders.get(id)?;
    loop {
        segments.push(current.name.clone());
        let Some(parent) = current.parent.as_deref() else {
            break;
        };
        current = state.folders.get(parent)?;
    }
    segments.reverse();
    Some(format!("{}{}", URI, canonical_segments(&segments)))
}

pub fn app_uri_in(state: &ApplicationsState, id: &str) -> Option<String> {
    let app = state.apps.get(id)?;
    let mut segments = Vec::new();
    let mut parent = app.parent.as_deref();
    while let Some(parent_id) = parent {
        let folder = state.folders.get(parent_id)?;
        segments.push(folder.name.clone());
        parent = folder.parent.as_deref();
    }
    segments.reverse();
    segments.push(app.display_name.clone());
    Some(format!("{}{}", URI, canonical_segments(&segments)))
}

pub fn parent_uri_matches(state: &ApplicationsState, uri: &str) -> bool {
    uri == URI || folder_id(state, uri).is_some()
}
