use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn copy_children(
    conn: &StoredConnection,
    mount: &Mount,
    ignore: &IgnoreSet,
    local: &Path,
    dest_parent: &str,
    existing: &HashMap<String, RemoteFile>,
    resolutions: &[Resolution],
    keep_both: bool,
    cancel: &AtomicBool,
    tasks: &mut Vec<FileTask>,
    dirs: &mut Vec<DirTask>,
    conflicts: &mut Vec<Conflict>,
    skipped: &mut usize,
    reserved: &mut HashSet<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(local).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        walk(
            conn,
            mount,
            ignore,
            &entry.path(),
            dest_parent,
            &name,
            existing,
            resolutions,
            keep_both,
            cancel,
            tasks,
            dirs,
            conflicts,
            skipped,
            reserved,
        )?;
    }
    Ok(())
}
/// Upload the sources into the connection owning `destination`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn upload(
    destination: &str,
    sources: &[Source],
    keep_both: bool,
    ignores: &[String],
    resolutions: &[Resolution],
    progress: &(dyn Fn(u64, u64, Option<&str>) + Sync),
    cancel: &AtomicBool,
) -> Result<OperationResult, String> {
    let (conn, relative) =
        connection_for_path(destination).ok_or("MountNotFound: connection removed")?;
    ensure_provider_access(&conn)?;
    let _guard = futures::executor::block_on(super::super::sync::lock(&conn));
    let mount = mount_for_path(destination)?;
    let dest_rel = rel_string(&relative);
    let meta = load_meta(&conn);
    let parent_node = meta
        .files
        .get(&dest_rel)
        .map(|file| file.node_id.clone())
        .filter(|node| !node.is_empty())
        .unwrap_or_else(|| dest_rel.clone());
    let existing: HashMap<String, RemoteFile> = list_remote(&conn, &dest_rel)?
        .into_iter()
        .map(|file| (file.name.clone(), file))
        .collect();

    let mut tasks = Vec::new();
    let mut dirs = Vec::new();
    let mut conflicts = Vec::new();
    let mut skipped = 0usize;
    let mut reserved: HashSet<String> = existing.keys().cloned().collect();
    let ignore = IgnoreSet::new(ignores);
    for source in sources {
        walk(
            &conn,
            &mount,
            &ignore,
            &source.local,
            &dest_rel,
            &source.name,
            &existing,
            resolutions,
            keep_both,
            cancel,
            &mut tasks,
            &mut dirs,
            &mut conflicts,
            &mut skipped,
            &mut reserved,
        )?;
    }
    if !conflicts.is_empty() {
        return Ok(OperationResult {
            paths: Vec::new(),
            errors: Vec::new(),
            skipped,
            conflicts,
        });
    }

    let mut meta = load_meta(&conn);
    let mut nodes: HashMap<String, String> = HashMap::new();
    nodes.insert(dest_rel.clone(), parent_node.clone());
    let mut created: Vec<String> = Vec::new();

    for dir in &dirs {
        check_cancelled(cancel)?;
        let parent_rel = dir.rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        let parent = nodes
            .get(parent_rel)
            .cloned()
            .unwrap_or_else(|| parent_rel.to_string());
        if let Some(node) = &dir.node {
            nodes.insert(dir.rel.clone(), node.clone());
            continue;
        }
        if let Some(existing) = &dir.existed {
            mount.remove(&existing.node_id)?;
        }
        let entry = mount.mkdir(&parent, &dir.name)?;
        nodes.insert(dir.rel.clone(), entry.node_id.clone());
        let mut file = RemoteMetaFile {
            is_dir: true,
            ..RemoteMetaFile::default()
        };
        remember(&entry, &mut file);
        meta.files.insert(dir.rel.clone(), file);
        created.push(navigation_path(&conn, &dir.rel)?);
    }

    let total = tasks.len() as u64;
    let mut errors = Vec::new();
    for (index, task) in tasks.iter().enumerate() {
        check_cancelled(cancel)?;
        progress(index as u64 + 1, total, Some(&task.name));
        if let Err(error) = upload_file(&conn, &mount, task, &mut nodes, &mut meta) {
            errors.push(format!("{}: {error}", task.local.display()));
        } else {
            created.push(navigation_path(&conn, &task.rel)?);
        }
        save_meta(&conn, &meta)?;
    }
    save_meta(&conn, &meta)?;
    Ok(OperationResult {
        paths: created,
        errors,
        skipped,
        conflicts: Vec::new(),
    })
}
