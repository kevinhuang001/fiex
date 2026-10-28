use super::*;

pub(super) fn remember(entry: &FsEntry, file: &mut RemoteMetaFile) {
    file.node_id = entry.node_id.clone();
    file.revision = entry.revision.clone().unwrap_or_default();
    file.is_dir = entry.is_dir();
    file.size = entry.size.unwrap_or_default();
    file.modified = entry.modified.unwrap_or(0).max(0) as u64;
    if let Some(readonly) = entry.readonly {
        file.readonly = readonly;
    }
    if let Some(operations) = &entry.allowed_ops {
        file.allowed_ops = Some(
            operations
                .iter()
                .map(|operation| operation.as_str().to_string())
                .collect(),
        );
    }
}

pub(super) fn upload_file(
    conn: &StoredConnection,
    mount: &Mount,
    task: &FileTask,
    nodes: &mut HashMap<String, String>,
    meta: &mut super::super::sync::RemoteMeta,
) -> Result<(), String> {
    let parent_rel = task.rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    let parent = nodes
        .get(parent_rel)
        .cloned()
        .unwrap_or_else(|| parent_rel.to_string());
    let mut input = fs::File::open(&task.local).map_err(|error| error.to_string())?;
    let (node_id, revision) = match &task.replace {
        Some(existing) if !existing.revision.is_empty() => (
            Some(existing.node_id.clone()),
            Some(existing.revision.clone()),
        ),
        Some(existing) => {
            mount.remove(&existing.node_id)?;
            (None, None)
        }
        None => (None, None),
    };
    let mut stream = mount
        .begin_write(WriteRequest {
            node_id: node_id.as_deref(),
            parent_id: if node_id.is_some() {
                None
            } else {
                Some(&parent)
            },
            name: if node_id.is_some() {
                None
            } else {
                Some(&task.name)
            },
            size: Some(task.size),
            if_revision: revision.as_deref(),
        })
        .map_err(|error| error.to_string())?;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        stream
            .write(&buffer[..read])
            .map_err(|error| error.to_string())?;
    }
    let committed = stream.commit().map_err(|error| error.to_string())?;
    let mut file = RemoteMetaFile {
        size: task.size,
        downloaded: false,
        uploaded: true,
        ..RemoteMetaFile::default()
    };
    if let Some(entry) = &committed {
        remember(entry, &mut file);
        nodes.insert(task.rel.clone(), entry.node_id.clone());
    } else if let Some(node) = node_id {
        nodes.insert(task.rel.clone(), node);
    }
    meta.files.insert(task.rel.clone(), file);
    let _ = conn;
    Ok(())
}
