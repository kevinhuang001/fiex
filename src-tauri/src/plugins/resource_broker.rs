//! Resource preparation shared by feature dispatchers. Metadata never contains a host path.
use crate::plugin_contracts::ResourceRef;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct Resource {
    pub local: PathBuf,
    pub reference: ResourceRef,
    pub is_dir: bool,
    pub ext: String,
    pub mime: String,
    pub metadata: Value,
}
impl Resource {
    pub fn new(display: &str, local: &Path) -> Self {
        let stat = std::fs::symlink_metadata(local).ok();
        let name = local
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let ext = local
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mime = crate::plugin_match::mime_for(&ext);
        let is_dir = stat.as_ref().is_some_and(|v| v.is_dir());
        let metadata = json!({
            "name": name, "kind": if is_dir { "directory" } else { "file" },
            "isDir": is_dir, "extension": ext, "mime": mime,
            "readonly": stat.as_ref().map(|v| v.permissions().readonly()),
            "symlink": stat.as_ref().is_some_and(|v| v.file_type().is_symlink()),
            "size": stat.as_ref().filter(|v| !v.is_dir()).map(|v| v.len()),
            "modified": stat.as_ref().and_then(|v| v.modified().ok())
                .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok()).map(|v| v.as_secs()),
        });
        Self {
            local: local.into(),
            reference: ResourceRef {
                mount_id: crate::mounts::parse(display)
                    .map(|uri| uri.mount_id)
                    .unwrap_or_else(|_| "local".into()),
                node_id: format!(
                    "fiex-resource:{}",
                    crate::mounts::new_id().expect("resource identity")
                ),
            },
            is_dir,
            ext,
            mime,
            metadata,
        }
    }
    /// Reuse directory-list/stat output without fetching remote contents.
    pub fn metadata_from_entry(entry: &crate::files::Entry) -> Value {
        json!({"name":entry.name,"kind":if entry.is_dir { "directory" } else { "file" },
            "isDir":entry.is_dir,"size":entry.size,"extension":entry.extension,"mime":entry.mime,
            "modified":entry.modified,"readonly":entry.readonly,"symlink":entry.is_symlink})
    }
    pub fn project(&self, metadata_allowed: bool) -> Value {
        let mut target = json!({"ref": self.reference});
        if metadata_allowed {
            target["metadata"] = self.metadata.clone();
        }
        target
    }
    pub fn request(&self, metadata_allowed: bool, options: Value) -> Value {
        json!({"target": self.project(metadata_allowed), "selection": [], "options": options})
    }
}
