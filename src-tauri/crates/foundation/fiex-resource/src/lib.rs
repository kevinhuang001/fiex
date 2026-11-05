//! Core-owned resource identity, snapshots, and capability projections.
//! This crate contains resource data only; routing and filesystem access are
//! supplied by the application layer.

use fiex_provider_contracts::{
    EventContext, FsOperation, ResourceCapabilities, ResourceContext, ResourceLocation,
    ResourceMetadata, ResourceRef,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileCapabilities {
    pub list: bool,
    pub stat: bool,
    pub search: bool,
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub rename: bool,
    pub trash: bool,
    pub remove: bool,
}

impl FileCapabilities {
    pub const ALL: Self = Self {
        list: true,
        stat: true,
        search: true,
        read: true,
        write: true,
        create: true,
        rename: true,
        trash: true,
        remove: true,
    };

    pub fn intersect(self, other: Self) -> Self {
        Self {
            list: self.list && other.list,
            stat: self.stat && other.stat,
            search: self.search && other.search,
            read: self.read && other.read,
            write: self.write && other.write,
            create: self.create && other.create,
            rename: self.rename && other.rename,
            trash: self.trash && other.trash,
            remove: self.remove && other.remove,
        }
    }

    pub fn allows(self, operation: FsOperation) -> bool {
        match operation {
            FsOperation::List => self.list,
            FsOperation::Stat => self.stat,
            FsOperation::Search => self.search,
            FsOperation::Read => self.read,
            FsOperation::Write => self.write,
            FsOperation::Mkdir => self.create,
            FsOperation::Rename => self.rename,
            FsOperation::Trash => self.trash,
            FsOperation::Remove => self.remove,
        }
    }

    pub fn has_mutation_capability(self) -> bool {
        self.write || self.create || self.rename || self.trash || self.remove
    }

    pub fn into_allowed_ops(self) -> Vec<String> {
        FsOperation::ALL
            .into_iter()
            .filter(|operation| self.allows(*operation))
            .map(|operation| operation.as_str().to_owned())
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
pub enum StorageProjection {
    #[default]
    Owned,
    Projected(Option<ResourceLocation>),
}

#[derive(Clone)]
pub struct ResourceSnapshot {
    pub display: String,
    pub local: PathBuf,
    pub reference: ResourceRef,
    pub stat: Option<fs::Metadata>,
    pub is_dir: bool,
    pub ext: String,
    pub mime: String,
    pub name: String,
    metadata: Option<ResourceMetadata>,
    pub operations: Option<FileCapabilities>,
    pub storage_projection: StorageProjection,
}

impl ResourceSnapshot {
    pub fn new(display: &str, local: &Path, mount_id: &str, mime: &str) -> Self {
        let stat = fs::symlink_metadata(local).ok();
        let is_dir = stat.as_ref().is_some_and(fs::Metadata::is_dir);
        let name = local
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| local.to_string_lossy().into_owned());
        let ext = Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mime = mime.to_owned();
        let reference = reference_for(display, mount_id);
        let metadata = stat.as_ref().map(|stat| ResourceMetadata {
            name: name.clone(),
            kind: if stat.file_type().is_symlink() {
                "symlink"
            } else if is_dir {
                "directory"
            } else {
                "file"
            }
            .into(),
            extension: (!ext.is_empty()).then(|| ext.clone()),
            mime: (!ext.is_empty()).then(|| mime.clone()),
            size: (!is_dir).then_some(stat.len()),
            modified: stat
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_secs() as i64),
        });
        Self {
            display: display.into(),
            local: local.to_path_buf(),
            reference,
            stat,
            is_dir,
            ext,
            mime,
            name,
            metadata,
            operations: None,
            storage_projection: StorageProjection::Owned,
        }
    }

    pub fn from_metadata(
        display: &str,
        local: PathBuf,
        mount_id: &str,
        metadata: ResourceMetadata,
    ) -> Self {
        let reference = reference_for(display, mount_id);
        Self {
            display: display.into(),
            local,
            reference,
            stat: None,
            is_dir: metadata.kind == "directory",
            ext: metadata.extension.clone().unwrap_or_default(),
            mime: metadata.mime.clone().unwrap_or_default(),
            name: metadata.name.clone(),
            metadata: Some(metadata),
            operations: None,
            storage_projection: StorageProjection::Owned,
        }
    }

    pub fn metadata(&self) -> Option<&ResourceMetadata> {
        self.metadata.as_ref()
    }

    pub fn project_data(
        &self,
        metadata_allowed: bool,
        view: Option<ResourceLocation>,
        storage: Option<ResourceLocation>,
        capabilities: ResourceCapabilities,
    ) -> ResourceContext {
        ResourceContext {
            reference: self.reference.clone(),
            metadata: metadata_allowed.then(|| self.metadata.clone()).flatten(),
            view,
            storage,
            capabilities: if metadata_allowed {
                capabilities
            } else {
                ResourceCapabilities::default()
            },
        }
    }

    pub fn event_context(&self, session_id: String, target: ResourceContext) -> EventContext {
        EventContext {
            targets: vec![target],
            session_id,
        }
    }
}

/// Stable within one Core process. The keyed digest does not reveal a host path.
/// This value is an identity only and never grants operation authorization.
pub fn reference_for(display: &str, mount_id: &str) -> ResourceRef {
    use sha2::{Digest, Sha256};
    use std::sync::OnceLock;

    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    let key = KEY.get_or_init(|| {
        let mut key = [0; 32];
        getrandom::getrandom(&mut key).expect("Core resource identity entropy unavailable");
        key
    });
    let mut digest = Sha256::new();
    digest.update(key);
    digest.update(mount_id.as_bytes());
    digest.update([0]);
    digest.update(display.as_bytes());
    ResourceRef {
        mount_id: mount_id.into(),
        node_id: format!("node:{}", hex::encode(digest.finalize())),
    }
}

/// Resource extension → MIME mapping shared by host and provider snapshots.
pub fn mime_for(ext: &str) -> String {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "tif" | "tiff" => "image/tiff",
        "avif" => "image/avif",
        "heic" => "image/heic",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "gz" | "tgz" => "application/gzip",
        "tar" => "application/x-tar",
        "bz2" => "application/x-bzip2",
        "xz" => "application/x-xz",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "aac" => "audio/aac",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "scss" => "text/x-scss",
        "less" => "text/x-less",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" | "rst" | "json5" | "toml" | "yaml" | "yml" | "ini" | "conf" | "cfg"
        | "env" | "sh" | "bat" | "ps1" | "py" | "rs" | "go" | "java" | "kt" | "c" | "cc"
        | "cpp" | "h" | "hpp" | "cs" | "php" | "rb" | "swift" | "sql" | "js" | "jsx" | "ts"
        | "tsx" | "vue" | "svelte" | "dart" | "lua" | "r" | "tex" => "text/plain",
        _ => "application/octet-stream",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_identity_is_scoped_to_mount_and_display() {
        let first = reference_for("applications://folder/app", "applications");
        assert_eq!(
            first,
            reference_for("applications://folder/app", "applications")
        );
        assert_ne!(
            first,
            reference_for("applications://folder/app", "another-mount")
        );
        assert_ne!(
            first,
            reference_for("applications://folder/other", "applications")
        );
    }

    #[test]
    fn mime_mapping_keeps_resource_preview_types() {
        assert_eq!(mime_for("pdf"), "application/pdf");
        assert_eq!(mime_for("mkv"), "video/x-matroska");
        assert_eq!(mime_for("unknown-extension"), "application/octet-stream");
    }

    #[test]
    fn capabilities_intersect_independent_operations() {
        let read_only = FileCapabilities {
            read: true,
            stat: true,
            ..FileCapabilities::default()
        };
        let writable = FileCapabilities {
            read: true,
            write: true,
            ..FileCapabilities::default()
        };
        assert!(read_only.intersect(writable).read);
        assert!(!read_only.intersect(writable).write);
    }
}
