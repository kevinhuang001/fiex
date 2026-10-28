use super::*;

pub fn owner(plugin_id: &str, _contribution: &str) -> Result<(), String> {
    let plugin = crate::plugins::host::catalog()
        .into_iter()
        .find(|p| p.id == plugin_id && p.enabled && p.approved && p.backend_available)
        .ok_or("PermissionDenied: VFS owner unavailable")?;
    if !plugin
        .extensions
        .get("vfs")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err("PermissionDenied: VFS provider declaration required".into());
    }
    Ok(())
}

pub fn open_view(plugin_id: &str, params: &Value, scope: &Scope) -> Result<Value, String> {
    let contribution = params["contributionId"]
        .as_str()
        .ok_or("missing contributionId")?;
    owner(plugin_id, contribution)?;
    let resource = params["resourceRoot"]
        .as_str()
        .ok_or("missing resourceRoot")?;
    scope.authorize(resource)?;
    let canonical = dunce::canonicalize(resource).map_err(|e| e.to_string())?;
    if !canonical.is_dir() {
        return Err("view resource must be a selected directory".into());
    }
    let context = params
        .get("viewContext")
        .cloned()
        .ok_or("missing viewContext")?;
    if !context.is_object() || context.to_string().len() > 16384 {
        return Err("invalid viewContext".into());
    }
    let descriptor = Descriptor {
        plugin_id: plugin_id.into(),
        contribution_id: plugin_id.into(),
        view_context: context,
        resource_root: canonical.to_string_lossy().into_owned(),
    };
    let mut state = registry()
        .lock()
        .map_err(|_| "mount registry unavailable")?;
    if let Some((id, _)) = state.views.iter().find(|(_, v)| {
        v.descriptor.plugin_id == plugin_id
            && v.descriptor.contribution_id == contribution
            && v.descriptor.resource_root == descriptor.resource_root
            && v.descriptor.view_context == descriptor.view_context
    }) {
        return Ok(serde_json::json!(uri(id, "")?));
    }
    collect(&mut state);
    if state.views.len() >= 256 {
        return Err("too many referenced VFS views".into());
    }
    let id = new_id()?;
    state.views.insert(
        id.clone(),
        View {
            descriptor,
            scope: scope.clone(),
            nodes: Arc::new(Mutex::new(HashMap::from([(String::new(), String::new())]))),
            created: Instant::now(),
        },
    );
    Ok(serde_json::json!(uri(&id, "")?))
}

pub(super) fn saved_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("fiex")
        .join("saved-vfs-views.json")
}

pub fn view(id: &str) -> Result<View, String> {
    if let Some(view) = registry()
        .lock()
        .map_err(|_| "mount registry unavailable")?
        .views
        .get(id)
        .cloned()
    {
        owner(&view.descriptor.plugin_id, &view.descriptor.contribution_id)?;
        view.scope.authorize(&view.descriptor.resource_root)?;
        return Ok(view);
    }
    // Only explicitly retained session/bookmark descriptors survive a restart.
    let saved: HashMap<String, Descriptor> = std::fs::read(saved_path())
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
        .unwrap_or_default();
    let descriptor = saved
        .get(id)
        .cloned()
        .ok_or("MountNotFound: view expired or removed")?;
    owner(&descriptor.plugin_id, &descriptor.contribution_id)?;
    let root = std::path::Path::new(&descriptor.resource_root);
    let scope = Scope::selected(root, &[root.to_path_buf()])?;
    let value = View {
        descriptor,
        scope,
        nodes: Arc::new(Mutex::new(HashMap::from([(String::new(), String::new())]))),
        created: Instant::now(),
    };
    registry()
        .lock()
        .map_err(|_| "mount registry unavailable")?
        .views
        .insert(id.into(), value.clone());
    Ok(value)
}

pub fn bind_node(id: &str, relative: &str, node: &str) -> Result<(), String> {
    let mut state = registry()
        .lock()
        .map_err(|_| "mount registry unavailable")?;
    let view = state.views.get_mut(id).ok_or("MountNotFound")?;
    let mut nodes = view.nodes.lock().map_err(|_| "node index unavailable")?;
    if nodes.len() >= 100_000 && !nodes.contains_key(relative) {
        return Err("too many VFS nodes".into());
    }
    if nodes.get(relative).is_some_and(|previous| previous != node) {
        return Err("provider changed a view node identity".into());
    }
    nodes.insert(relative.into(), node.into());
    Ok(())
}

pub(super) fn collect(state: &mut Registry) {
    let live: HashSet<_> = state
        .references
        .values()
        .flat_map(|(a, b)| a.union(b).cloned())
        .collect();
    state
        .views
        .retain(|id, v| live.contains(id) || v.created.elapsed().as_secs() < 60);
}

pub fn retain(window: &str, paths: &[String], durable: &[String]) -> Result<(), String> {
    let ids = |values: &[String]| {
        values
            .iter()
            .filter_map(|p| parse(p).ok().map(|u| u.mount_id))
            .collect::<HashSet<_>>()
    };
    let live = ids(paths);
    let durable = ids(durable);
    let previous: HashMap<String, Descriptor> = std::fs::read(saved_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    // Load retained descriptors before replacing the previous saved session.
    let existing: HashSet<String> = registry()
        .lock()
        .map_err(|_| "mount registry unavailable")?
        .views
        .keys()
        .cloned()
        .collect();
    for id in live.union(&durable) {
        if !existing.contains(id) && previous.contains_key(id) {
            let _ = view(id);
        }
    }
    let mut state = registry()
        .lock()
        .map_err(|_| "mount registry unavailable")?;
    state.references.insert(window.into(), (live, durable));
    collect(&mut state);
    let saved: HashMap<_, _> = state
        .references
        .values()
        .flat_map(|(_, ids)| ids)
        .filter_map(|id| {
            state
                .views
                .get(id)
                .map(|v| (id.clone(), v.descriptor.clone()))
                .or_else(|| {
                    previous
                        .get(id)
                        .cloned()
                        .map(|descriptor| (id.clone(), descriptor))
                })
        })
        .collect();
    if saved == previous {
        return Ok(());
    }
    let path = saved_path();
    let parent = path.parent().ok_or("invalid VFS state directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    use std::io::Write;
    file.write_all(&serde_json::to_vec(&saved).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn forget_window(window: &str) {
    if let Ok(mut state) = registry().lock() {
        state.references.remove(window);
        collect(&mut state);
    }
}
