use super::*;

pub(super) fn relative(root: &Scope, input: &str) -> Result<PathBuf, String> {
    let path = Path::new(input);
    let path = if path.is_absolute() {
        path.strip_prefix(&root.path)
            .map_err(|_| "path outside task directory")?
    } else {
        path
    };
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err("path outside task directory".into());
    }
    let path = if path.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        path.into()
    };
    if let Some(allowed) = &root.allowed {
        let resolved = match root.dir.canonicalize(&path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // A planned directory copy can authorize descendants before
                // their parents are created. Resolve the nearest existing
                // ancestor through the directory capability, then append only
                // normal path components from the absent tail.
                let mut ancestor = path.as_path();
                let mut missing = Vec::new();
                loop {
                    match root.dir.canonicalize(ancestor) {
                        Ok(base) => {
                            break missing
                                .into_iter()
                                .rev()
                                .fold(base, |base: PathBuf, name| base.join(name))
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            missing.push(ancestor.file_name().ok_or("invalid selected path")?);
                            ancestor = ancestor.parent().unwrap_or(Path::new("."));
                        }
                        Err(e) => return Err(e.to_string()),
                    }
                }
            }
            Err(error) => return Err(error.to_string()),
        };
        let normalize = |candidate: &Path| {
            let normalized: PathBuf = candidate
                .components()
                .filter(|part| matches!(part, Component::Normal(_)))
                .collect();
            if normalized.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                normalized
            }
        };
        let resolved = normalize(&resolved);
        let permitted = |candidate: &Path| {
            allowed.iter().any(|(selected, is_dir)| {
                candidate == selected
                    || (*is_dir && (selected == Path::new(".") || candidate.starts_with(selected)))
            })
        };
        if !permitted(&path) || !permitted(&resolved) {
            return Err("path not selected for this operation".into());
        }
    }
    Ok(path)
}
pub(super) fn directory(root: &Scope) -> Result<Dir, String> {
    root.dir.try_clone().map_err(|e| e.to_string())
}
pub(super) fn entry(dir: &Dir, path: &Path) -> Result<Value, String> {
    let metadata = dir.metadata(path).map_err(|e| e.to_string())?;
    Ok(
        json!({"path": path.to_string_lossy(), "name": path.file_name().unwrap_or_default().to_string_lossy(), "isDir": metadata.is_dir(), "size": metadata.len(), "modified": metadata.modified().ok().and_then(|t| t.into_std().duration_since(std::time::UNIX_EPOCH).ok()).map(|t| t.as_secs()).unwrap_or(0), "extension": path.extension().unwrap_or_default().to_string_lossy(), "mime": if metadata.is_dir() { "inode/directory".into() } else { mime_guess::from_path(path).first_or_octet_stream().to_string() }, "readonly": metadata.permissions().readonly(), "symlink": dir.symlink_metadata(path).map(|m| m.is_symlink()).unwrap_or(false)}),
    )
}
pub fn list(root: &Scope, input: &str) -> Result<Value, String> {
    let dir = directory(root)?;
    let path = relative(root, input)?;
    let mut values = Vec::new();
    for item in dir.read_dir(&path).map_err(|e| e.to_string())?.take(5001) {
        if values.len() == 5000 {
            return Err("directory exceeds listing limit".into());
        }
        let item = item.map_err(|e| e.to_string())?;
        // Omit symlinks whose targets cannot be resolved inside the capability.
        let child = path.join(item.file_name());
        if let Ok(value) = entry(&dir, &child) {
            values.push(value);
        }
    }
    values.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_lowercase());
    Ok(json!(values))
}
pub fn stat(root: &Scope, input: &str) -> Result<Value, String> {
    entry(&directory(root)?, &relative(root, input)?)
}
pub fn is_directory(root: &Scope, input: &str) -> Result<bool, String> {
    let path = relative(root, input)?;
    let metadata = directory(root)?
        .symlink_metadata(path)
        .map_err(|e| e.to_string())?;
    Ok(metadata.is_dir())
}
/// Preserve input order and isolate failures so one inaccessible selection does
/// not disclose its attributes or prevent metadata for the other selections.
pub fn stat_batch(root: &Scope, paths: &[Value]) -> Result<Value, String> {
    if paths.len() > 256 {
        return Err("stat batch exceeds 256 entries".into());
    }
    let dir = directory(root)?;
    let mut seen = std::collections::HashSet::new();
    let results: Vec<Value> = paths
        .iter()
        .map(|value| {
            let Some(path) = value.as_str() else {
                return json!({"error": "invalid path"});
            };
            let Ok(relative) = relative(root, path) else {
                return json!({"error": "permission denied"});
            };
            if !seen.insert(relative.clone()) {
                return json!({"error": "duplicate selection"});
            }
            match entry(&dir, &relative) {
                Ok(metadata) => json!({"metadata": metadata}),
                Err(_) => json!({"error": "metadata unavailable"}),
            }
        })
        .collect();
    Ok(json!({"results": results}))
}
pub fn mkdir(root: &Scope, input: &str) -> Result<Value, String> {
    let dir = directory(root)?;
    let path = relative(root, input)?;
    dir.create_dir_all(&path).map_err(|e| e.to_string())?;
    entry(&dir, &path)
}
pub fn read_chunk(root: &Scope, input: &str, offset: u64, length: usize) -> Result<Value, String> {
    let mut file = directory(root)?
        .open(relative(root, input)?)
        .map_err(|e| e.to_string())?;
    let total = file.metadata().map_err(|e| e.to_string())?.len();
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut data = vec![0; length.min(MAX_CHUNK)];
    let read = file.read(&mut data).map_err(|e| e.to_string())?;
    data.truncate(read);
    Ok(
        json!({"base64": STANDARD.encode(&data), "size": read, "eof": offset.saturating_add(read as u64) >= total}),
    )
}
pub fn remove(root: &Scope, input: &str) -> Result<(), String> {
    let path = relative(root, input)?;
    if !path.components().any(|c| matches!(c, Component::Normal(_))) {
        return Err("cannot remove task root".into());
    }
    let dir = directory(root)?;
    let metadata = dir.symlink_metadata(&path).map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        dir.remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        dir.remove_file(path).map_err(|e| e.to_string())
    }
}
pub fn rename(root: &Scope, from: &str, to: &str) -> Result<(), String> {
    let dir = directory(root)?;
    let from = relative(root, from)?;
    let to = relative(root, to)?;
    if !from.components().any(|c| matches!(c, Component::Normal(_)))
        || !to.components().any(|c| matches!(c, Component::Normal(_)))
    {
        return Err("cannot rename task root".into());
    }
    if dir.symlink_metadata(&to).is_ok() {
        return Err("rename destination already exists".into());
    }
    dir.rename(from, &dir, to).map_err(|e| e.to_string())
}
