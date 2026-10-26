use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    pub color: u8,
}

#[cfg(target_os = "macos")]
const TAG_XATTR: &str = "com.apple.metadata:_kMDItemUserTags";

#[cfg(target_os = "macos")]
fn parse_tag(value: &str) -> Option<Tag> {
    let (name, color) = match value.rsplit_once('\n') {
        Some((name, color)) => (name, color.parse::<u8>().ok()?.min(7)),
        None => (value, 0),
    };
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some(Tag {
        name: name.to_string(),
        color,
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn read_tags(path: &Path) -> Vec<Tag> {
    let Some(bytes) = xattr::get(path, TAG_XATTR).ok().flatten() else {
        return Vec::new();
    };
    let Ok(value) = plist::Value::from_reader(std::io::Cursor::new(bytes)) else {
        return Vec::new();
    };
    match value.as_array() {
        Some(items) => items
            .iter()
            .filter_map(|item| item.as_string())
            .filter_map(parse_tag)
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn read_tags(_path: &Path) -> Vec<Tag> {
    Vec::new()
}

#[cfg(target_os = "macos")]
pub fn set_tags(paths: &[String], tags: &[Tag]) -> Result<(), String> {
    let mut values = Vec::new();
    for tag in tags {
        let name = tag.name.trim();
        if name.is_empty() || name.contains('\n') {
            continue;
        }
        values.push(plist::Value::String(format!(
            "{name}\n{}",
            tag.color.min(7)
        )));
    }
    let mut bytes = Vec::new();
    plist::Value::Array(values)
        .to_writer_binary(&mut bytes)
        .map_err(|e| e.to_string())?;
    for path in paths {
        let path = Path::new(path);
        if !path.exists() {
            continue;
        }
        if tags.is_empty() {
            match xattr::remove(path, TAG_XATTR) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        } else {
            xattr::set(path, TAG_XATTR, &bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_tags(_paths: &[String], _tags: &[Tag]) -> Result<(), String> {
    Err(crate::i18n::t("Finder 标签仅在 macOS 上可用").into())
}
