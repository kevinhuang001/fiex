use super::super::{decode, encode, URI, USER_PREFIX};
use std::path::{Path, PathBuf};

pub(in crate::core::volumes) fn user_encoded(uri: &str) -> Option<&str> {
    uri.strip_prefix(USER_PREFIX)
}

pub fn is_user(path: &str) -> bool {
    user_encoded(path).is_some()
}

fn user_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".fiex")
        .join("applications")
}

/// The writable side of Applications is a sandboxed local store. URI payloads
/// are relative to that store, including nested user-created directories.
pub fn stored_path(uri: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(user_dir()).map_err(|e| e.to_string())?;
    let root = dunce::canonicalize(user_dir()).map_err(|e| e.to_string())?;
    if uri == URI {
        return Ok(root);
    }
    let relative = decode(user_encoded(uri).ok_or("not a stored application")?)?;
    if relative.is_empty()
        || relative.contains(['\\', ':', '\0'])
        || Path::new(&relative)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("invalid application store path".into());
    }
    let target = root.join(relative);
    let canonical = dunce::canonicalize(&target).map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root) || canonical == root {
        return Err("application store escape".into());
    }
    Ok(canonical)
}

/// Validate an Applications user URI without requiring the final node to
/// exist. A newly created child is confined to the canonical private root;
/// every existing ancestor is canonicalized to reject symlink escapes.
pub fn candidate_path(uri: &str) -> Result<PathBuf, String> {
    let root = stored_path(URI)?;
    let encoded = user_encoded(uri).ok_or("not a user application")?;
    let relative = decode(encoded)?;
    if relative.is_empty()
        || relative.contains(['\\', ':', '\0'])
        || Path::new(&relative)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("invalid application store path".into());
    }
    let target = root.join(relative);
    let mut ancestor = target.as_path();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(
            ancestor
                .file_name()
                .ok_or("invalid application child path")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().ok_or("missing application ancestor")?;
    }
    let mut resolved = dunce::canonicalize(ancestor).map_err(|e| e.to_string())?;
    if !resolved.starts_with(&root) {
        return Err("application store escape".into());
    }
    for child in missing.iter().rev() {
        resolved.push(child);
    }
    if resolved == root {
        return Err("invalid application child path".into());
    }
    Ok(resolved)
}

/// Return the lexical path of a user entry without resolving the final
/// component. Classification and recursive safety checks use this form so a
/// symlink cannot be mistaken for its target or traversed accidentally.
pub fn lstat_path(uri: &str) -> Result<PathBuf, String> {
    let root = stored_path(URI)?;
    let encoded = user_encoded(uri).ok_or("not a user application")?;
    let relative = decode(encoded)?;
    if relative.is_empty()
        || relative.contains(['\\', ':', '\0'])
        || Path::new(&relative)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("invalid application store path".into());
    }
    Ok(root.join(relative))
}

/// Ensure a generated child belongs directly to the selected Applications
/// directory. This is used when Core binds a provider-returned ResourceRef.
pub fn validate_child(parent: &str, child: &str) -> Result<(), String> {
    let parent_path = if parent == URI {
        stored_path(parent)?
    } else if is_user(parent) {
        candidate_path(parent)?
    } else {
        return Err("ScopeDenied: invalid application parent".into());
    };
    let child_path = candidate_path(child)?;
    if child_path.parent() != Some(parent_path.as_path()) {
        return Err("ScopeDenied: child outside selected application directory".into());
    }
    Ok(())
}

pub fn stored_uri(path: &Path) -> Result<String, String> {
    let root = dunce::canonicalize(user_dir()).map_err(|e| e.to_string())?;
    let rel = path
        .strip_prefix(root)
        .map_err(|_| "not in application store")?;
    Ok(format!(
        "{USER_PREFIX}{}",
        encode(&rel.to_string_lossy().replace('\\', "/"))
    ))
}
