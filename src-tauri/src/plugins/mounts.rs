//! Core-owned navigation identities. URIs locate resources; they never grant access.
use crate::plugin_fs_access::Scope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uri {
    pub mount_id: String,
    pub relative: String,
}
pub fn handles(path: &str) -> bool {
    path.get(..5)
        .is_some_and(|s| s.eq_ignore_ascii_case("fiex:"))
}
pub fn segment(part: &str) -> Result<(), String> {
    if part.is_empty()
        || part.len() > 1024
        || matches!(part, "." | "..")
        || part.contains(['/', '\\', '\0', ':'])
        || part.chars().any(char::is_control)
    {
        return Err("InvalidVfsUri: invalid path segment".into());
    }
    // Reject nested encodings of traversal/separators, while allowing a literal %.
    let lower = part.to_ascii_lowercase();
    if ["%2e", "%2f", "%5c", "%00", "%25"]
        .iter()
        .any(|v| lower.contains(v))
    {
        return Err("InvalidVfsUri: nested path encoding".into());
    }
    Ok(())
}
pub fn parse(path: &str) -> Result<Uri, String> {
    if path.len() > 32768 {
        return Err("InvalidVfsUri: URI too long".into());
    }
    let rest = path
        .strip_prefix("fiex://mount/")
        .ok_or("InvalidVfsUri: expected fiex://mount/")?;
    let (id, relative) = rest
        .split_once('/')
        .ok_or("InvalidVfsUri: missing root slash")?;
    if id.is_empty()
        || id.len() > 128
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("InvalidVfsUri: invalid mount ID".into());
    }
    let mut decoded = Vec::new();
    if !relative.is_empty() {
        if relative.split('/').count() > 256 {
            return Err("InvalidVfsUri: too many path segments".into());
        }
        for part in relative.split('/') {
            let bytes = part.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'%' {
                    if i + 2 >= bytes.len()
                        || !bytes[i + 1].is_ascii_hexdigit()
                        || !bytes[i + 2].is_ascii_hexdigit()
                    {
                        return Err("InvalidVfsUri: invalid encoding".into());
                    }
                    i += 2;
                }
                i += 1;
            }
            if part.contains(['?', '#', '\\']) {
                return Err("InvalidVfsUri: unescaped delimiter".into());
            }
            let value = urlencoding::decode(part)
                .map_err(|_| "InvalidVfsUri: invalid UTF-8")?
                .into_owned();
            segment(&value)?;
            decoded.push(value);
        }
    }
    Ok(Uri {
        mount_id: id.into(),
        relative: decoded.join("/"),
    })
}
pub fn uri(id: &str, relative: &str) -> Result<String, String> {
    let mut result = format!("fiex://mount/{id}/");
    if !relative.is_empty() {
        let mut encoded = Vec::new();
        for part in relative.split('/') {
            segment(part)?;
            encoded.push(urlencoding::encode(part).into_owned());
        }
        result.push_str(&encoded.join("/"));
    }
    parse(&result)?;
    Ok(result)
}
pub fn new_id() -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let token = tempfile::Builder::new()
        .rand_bytes(24)
        .tempfile()
        .map_err(|e| e.to_string())?;
    Ok(hex::encode(Sha256::digest(
        token.path().to_string_lossy().as_bytes(),
    )))
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    pub plugin_id: String,
    pub contribution_id: String,
    pub view_context: Value,
    pub resource_root: String,
}
#[derive(Clone)]
pub struct View {
    pub descriptor: Descriptor,
    pub scope: Scope,
    pub nodes: Arc<Mutex<HashMap<String, String>>>,
    created: Instant,
}
impl View {
    pub fn node_id(&self, relative: &str) -> Result<Option<String>, String> {
        self.nodes
            .lock()
            .map(|nodes| nodes.get(relative).cloned())
            .map_err(|_| "node index unavailable".into())
    }
}
#[derive(Default)]
struct Registry {
    views: HashMap<String, View>,
    references: HashMap<String, (HashSet<String>, HashSet<String>)>,
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}
pub fn owner(plugin_id: &str, contribution: &str) -> Result<(), String> {
    let plugin = crate::plugin_host::catalog()
        .into_iter()
        .find(|p| p.id == plugin_id && p.enabled && p.approved && p.backend_available)
        .ok_or("PermissionDenied: VFS owner unavailable")?;
    if !plugin
        .extensions
        .get("vfs")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|v| v["id"].as_str() == Some(contribution)))
    {
        return Err("PermissionDenied: explicit VFS contribution required".into());
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
        contribution_id: contribution.into(),
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
fn saved_path() -> std::path::PathBuf {
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
fn collect(state: &mut Registry) {
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
pub fn reject_local(path: &str) -> Result<(), String> {
    if handles(path) {
        parse(path)?;
        return Err(
            "VFS resource requires Core routing; this local operation is unavailable".into(),
        );
    }
    if path.starts_with("history://") {
        return Err("旧历史地址已失效，请在 Git 面板重新选择提交并保存标签页/书签".into());
    }
    Ok(())
}
/// The sole routing decision for Core navigation URIs. Cache-path recognition
/// remains an internal remote adapter detail, not a second navigation scheme.
pub enum Source {
    Connection(crate::remote::StoredConnection),
    View(View),
}
pub fn resolve(path: &str) -> Result<(Uri, Source), String> {
    let uri = parse(path)?;
    if let Some(conn) = crate::remote::connection_by_mount(&uri.mount_id) {
        crate::remote::ensure_provider_access(&conn)?;
        return Ok((uri, Source::Connection(conn)));
    }
    let view = view(&uri.mount_id)?;
    Ok((uri, Source::View(view)))
}
pub fn forget_window(window: &str) {
    if let Ok(mut state) = registry().lock() {
        state.references.remove(window);
        collect(&mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_view(root: &std::path::Path) -> View {
        View {
            descriptor: Descriptor {
                plugin_id: "test.owner".into(),
                contribution_id: "tree".into(),
                view_context: serde_json::json!({"object":"immutable"}),
                resource_root: root.to_string_lossy().into_owned(),
            },
            scope: Scope::new(root).unwrap(),
            nodes: Arc::new(Mutex::new(HashMap::new())),
            created: Instant::now() - std::time::Duration::from_secs(61),
        }
    }
    #[test]
    fn collection_preserves_all_window_references_without_persisting_history() {
        let root = tempfile::tempdir().unwrap();
        let mut state = Registry::default();
        for id in ["tab", "back", "bookmark", "unreferenced"] {
            state.views.insert(id.into(), test_view(root.path()));
        }
        state.references.insert(
            "window-a".into(),
            (HashSet::from(["tab".into(), "back".into()]), HashSet::new()),
        );
        state.references.insert(
            "window-b".into(),
            (HashSet::new(), HashSet::from(["bookmark".into()])),
        );
        collect(&mut state);
        assert_eq!(state.views.len(), 3);
        assert!(!state.views.contains_key("unreferenced"));
        state.references.remove("window-a");
        collect(&mut state);
        assert_eq!(state.views.len(), 1);
        assert!(state.views.contains_key("bookmark"));
        let descriptor = &state.views["bookmark"].descriptor;
        let saved = serde_json::to_value(descriptor).unwrap();
        assert!(saved.get("nodes").is_none());
        assert_eq!(saved["viewContext"]["object"], "immutable");
    }
    #[test]
    fn uri_roundtrip_and_mount_isolation() {
        for name in ["hello world", "中文", "#?%", "a.txt"] {
            let path = uri("abc-123", name).unwrap();
            assert_eq!(parse(&path).unwrap().relative, name);
            assert_eq!(parse(&path).unwrap().mount_id, "abc-123");
        }
        assert_ne!(uri("one", "x").unwrap(), uri("two", "x").unwrap());
    }
    #[test]
    fn malformed_addresses_never_fall_back_to_local_paths() {
        for path in [
            "fiex://other/x/",
            "fiex://mount/x",
            "fiex://mount/x/..",
            "fiex://mount/x/%2e%2e",
            "fiex://mount/x/%252e%252e",
            "fiex://mount/x/a%2fb",
            "fiex://mount/x/a%5cb",
            "fiex://mount/x/%00",
            "fiex://mount/x/%zz",
            "fiex://mount/x/a?b",
            "fiex://mount/x/a//b",
            "FIEX://mount/x/",
        ] {
            assert!(handles(path));
            assert!(parse(path).is_err(), "{path}");
            assert!(reject_local(path).is_err());
        }
        assert!(view("nonexistent-test-mount").is_err());
    }
}
