/// A save dialog may select a file that does not exist yet. Canonicalize its
/// existing parent so a symlinked parent cannot change the selected target.
use super::*;
use fiex_scope_policy::{MethodDenial, method_denial};

mod mapped;
mod references;
use mapped::MappedResourceBinding;

pub fn canonical_selection(path: &Path) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("selection must not contain parent traversal".into());
    }
    let mut ancestor = path;
    let mut missing = Vec::new();
    loop {
        match dunce::canonicalize(ancestor) {
            Ok(base) => {
                return Ok(missing
                    .into_iter()
                    .rev()
                    .fold(base, |base: PathBuf, name| base.join(name)));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or("selection has no name")?
                        .to_os_string(),
                );
                ancestor = ancestor.parent().ok_or("selection has no parent")?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// Captured once at invocation start, including an open directory capability.
#[derive(Clone)]
pub struct Scope {
    pub(super) path: PathBuf,
    pub(super) dir: std::sync::Arc<Dir>,
    pub(super) allowed: Option<Vec<(PathBuf, bool)>>,
    pub(super) references: std::sync::Arc<HashMap<String, String>>,
    pub(super) metadata_refs: std::sync::Arc<HashMap<String, Value>>,
    pub(super) metadata_only: bool,
    pub(super) read_only: bool,
    pub(super) issued: std::sync::Arc<
        Mutex<HashMap<String, crate::plugins::resource::resource_context::ResourceSnapshot>>,
    >,
    pub(super) issued_capabilities:
        std::sync::Arc<Mutex<fiex_scope_policy::DerivedReferenceGrants>>,
    pub(super) remote_resources: std::sync::Arc<
        HashMap<String, crate::plugins::resource::resource_context::ResourceSnapshot>,
    >,
    user_action: Option<crate::core::fs::mapped_access::UserActionContext>,
    mapped_resources: std::sync::Arc<Mutex<HashMap<String, MappedResourceBinding>>>,
}
impl Scope {
    pub fn new(path: &Path) -> Result<Self, String> {
        let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
        let dir = Dir::open_ambient_dir(&path, ambient_authority()).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            dir: std::sync::Arc::new(dir),
            allowed: None,
            references: Default::default(),
            metadata_refs: Default::default(),
            metadata_only: false,
            read_only: false,
            issued: Default::default(),
            issued_capabilities: Default::default(),
            remote_resources: Default::default(),
            user_action: None,
            mapped_resources: Default::default(),
        })
    }
    /// Capture only paths chosen for this operation. A file grants that file;
    /// a directory grants its descendants. An empty selection grants no files.
    pub fn selected(path: &Path, paths: &[PathBuf]) -> Result<Self, String> {
        let mut scope = Self::new(path)?;
        let mut allowed = Vec::with_capacity(paths.len());
        for selected in paths {
            let canonical = canonical_selection(selected)?;
            let relative = canonical
                .strip_prefix(&scope.path)
                .map_err(|_| "selection outside task directory")?
                .to_path_buf();
            let relative = if relative.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                relative
            };
            let is_dir = scope.dir.metadata(&relative).is_ok_and(|m| m.is_dir());
            allowed.push((relative, is_dir));
        }
        scope.allowed = Some(allowed);
        Ok(scope)
    }
    pub fn restrict_to_metadata(&mut self) {
        self.metadata_only = true;
    }
    pub fn restrict_to_readonly(&mut self) {
        self.read_only = true;
    }
    pub fn authorize_method(&self, method: &str) -> Result<(), String> {
        match method_denial(method, self.metadata_only, self.read_only) {
            Some(MethodDenial::UnsupportedFilesystemNamespace) => {
                Err("unsupported filesystem method namespace".into())
            }
            Some(MethodDenial::ReadOnly) => {
                Err("PermissionDenied: view does not support writes".into())
            }
            Some(MethodDenial::MetadataOnly) => Err("discovery scope permits metadata only".into()),
            None => Ok(()),
        }
    }
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        self.allowed
            .as_ref()
            .map(|paths| paths.iter().map(|(p, _)| self.path.join(p)).collect())
            .unwrap_or_default()
    }
    fn ref_key(reference: &Value) -> Result<String, String> {
        let reference: crate::plugins::contracts::ResourceRef =
            serde_json::from_value(reference.clone()).map_err(|e| e.to_string())?;
        serde_json::to_string(&reference).map_err(|e| e.to_string())
    }
    pub fn authorize(&self, path: &str) -> Result<(), String> {
        if self.mapped_path_authorized(path)? {
            return Ok(());
        }
        relative(self, path).map(|_| ())
    }

    pub(super) fn issued_reference_active(&self, key: &str) -> Result<bool, String> {
        let exists = self
            .issued
            .lock()
            .map_err(|_| "reference registry unavailable")?
            .contains_key(key);
        if !exists {
            return Ok(false);
        }
        Ok(self
            .issued_capabilities
            .lock()
            .map_err(|_| "reference capability registry unavailable")?
            .is_active(key))
    }

    /// Revoke an identity derived by this operation. Its descendants become
    /// stale in every cloned Scope that shares the operation registry.
    pub fn revoke_issued_reference(&self, reference: &Value) -> Result<bool, String> {
        let key = Self::ref_key(reference)?;
        Ok(self
            .issued_capabilities
            .lock()
            .map_err(|_| "reference capability registry unavailable")?
            .revoke(&key))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
