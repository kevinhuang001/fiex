//! Stable Trash identities over the platform Recycle Bin backend.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
#[cfg(windows)]
use fiex_platform::recycle_bin::RecycleBinItemHandle;
use fiex_platform::recycle_bin::{
    RecycleBinBackend, RecycleBinItemMetadata, SystemRecycleBinBackend,
};
use fiex_provider_contracts::{FsEntry, FsKind, FsListing, FsOperation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const RECYCLE_BIN_URI: &str = "recyclebin://";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ItemIdentity {
    id: String,
    name: String,
    original: String,
    deleted_unix: i64,
    folder: bool,
}

impl From<&RecycleBinItemMetadata> for ItemIdentity {
    fn from(item: &RecycleBinItemMetadata) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            original: item.original.clone(),
            deleted_unix: item.deleted_unix,
            folder: item.folder,
        }
    }
}

impl From<ItemIdentity> for RecycleBinItemMetadata {
    fn from(identity: ItemIdentity) -> Self {
        Self {
            id: identity.id,
            name: identity.name,
            original: identity.original,
            deleted_unix: identity.deleted_unix,
            folder: identity.folder,
        }
    }
}

pub struct RecycleBin {
    backend: Arc<dyn RecycleBinBackend>,
}

impl RecycleBin {
    pub fn new(backend: Arc<dyn RecycleBinBackend>) -> Self {
        Self { backend }
    }

    pub fn system() -> Self {
        Self::new(Arc::new(SystemRecycleBinBackend))
    }

    pub fn list(&self) -> Result<FsListing, String> {
        let entries = self
            .backend
            .enumerate()?
            .iter()
            .map(|item| entry(item.metadata()))
            .collect();
        Ok(FsListing {
            entries,
            ..Default::default()
        })
    }

    #[cfg(windows)]
    pub fn stat(&self, uri: &str) -> Result<FsEntry, String> {
        let encoded = encoded_identity(uri)?;
        let (expected, _) = self.resolve_item(encoded)?;
        let metadata = RecycleBinItemMetadata::from(expected);
        let resolved = entry(&metadata);
        if resolved.node_id != uri {
            return Err("StaleRef: Recycle Bin item identity changed".into());
        }
        Ok(resolved.with_revision(metadata.deleted_unix.to_string()))
    }

    #[cfg(not(windows))]
    pub fn stat(&self, _uri: &str) -> Result<FsEntry, String> {
        Err("Unsupported: Recycle Bin item is unavailable".into())
    }

    #[cfg(windows)]
    pub fn restore(&self, uri: &str) -> Result<(), String> {
        let encoded = encoded_identity(uri)?;
        let (_, item) = self.resolve_item(encoded)?;
        item.restore()
    }

    #[cfg(not(windows))]
    pub fn restore(&self, _uri: &str) -> Result<(), String> {
        Err("Unsupported: Recycle Bin item is unavailable".into())
    }

    #[cfg(windows)]
    pub fn purge(&self, uri: &str) -> Result<(), String> {
        let encoded = encoded_identity(uri)?;
        let (_, item) = self.resolve_item(encoded)?;
        item.purge()
    }

    #[cfg(not(windows))]
    pub fn purge(&self, _uri: &str) -> Result<(), String> {
        Err("Unsupported: Recycle Bin item is unavailable".into())
    }

    pub fn empty(&self) -> Result<usize, String> {
        self.backend.empty()
    }

    #[cfg(windows)]
    fn resolve_item(
        &self,
        encoded: &str,
    ) -> Result<(ItemIdentity, Box<dyn RecycleBinItemHandle>), String> {
        let expected = decode(encoded)?;
        let mut matches = self
            .backend
            .enumerate()?
            .into_iter()
            .filter(|item| item.metadata().id == expected.id);
        let item = matches
            .next()
            .ok_or_else(|| "StaleRef: Recycle Bin item no longer exists".to_owned())?;
        if matches.next().is_some() {
            return Err("StaleRef: Recycle Bin item identity is ambiguous".into());
        }
        if ItemIdentity::from(item.metadata()) != expected {
            return Err("StaleRef: Recycle Bin item changed since enumeration".into());
        }
        Ok((expected, item))
    }
}

pub fn is_virtual(uri: &str) -> bool {
    uri == RECYCLE_BIN_URI || uri.starts_with(RECYCLE_BIN_URI)
}

fn entry(item: &RecycleBinItemMetadata) -> FsEntry {
    let extension = item
        .name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_lowercase());
    let identity = ItemIdentity::from(item);
    let encoded = encode(&identity);
    FsEntry {
        node_id: format!(
            "{RECYCLE_BIN_URI}{}?id={encoded}",
            crate::encode_uri_segment(&item.name)
        ),
        name: item.name.clone(),
        kind: if item.folder {
            FsKind::Directory
        } else {
            FsKind::File
        },
        extension,
        link_target: (!item.original.is_empty()).then(|| item.original.clone()),
        allowed_ops: Some(vec![FsOperation::Stat, FsOperation::Remove]),
        revision: Some(item.deleted_unix.to_string()),
        ..Default::default()
    }
}

#[cfg(windows)]
fn encoded_identity(uri: &str) -> Result<&str, String> {
    let (name, query) = uri
        .strip_prefix(RECYCLE_BIN_URI)
        .and_then(|value| value.split_once('?'))
        .ok_or_else(|| "InvalidVfsUri: invalid Recycle Bin item".to_owned())?;
    let encoded = query
        .strip_prefix("id=")
        .filter(|encoded| {
            !encoded.is_empty()
                && !encoded.contains('&')
                && !encoded.contains('/')
                && !encoded.contains('?')
        })
        .ok_or_else(|| "InvalidVfsUri: invalid Recycle Bin item".to_owned())?;
    let identity = decode(encoded)?;
    if name.is_empty() || name.contains('/') || crate::encode_uri_segment(&identity.name) != name {
        return Err("InvalidVfsUri: invalid Recycle Bin item".into());
    }
    Ok(encoded)
}

fn encode(identity: &ItemIdentity) -> String {
    let bytes = serde_json::to_vec(identity).expect("Recycle Bin identity serializes");
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(any(windows, test))]
fn decode(encoded: &str) -> Result<ItemIdentity, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| "InvalidVfsUri: invalid Recycle Bin item")?;
    serde_json::from_slice(&bytes).map_err(|_| "回收站项目无效".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recycle_bin_uri_contains_an_opaque_identity() {
        let item = RecycleBinItemMetadata {
            id: "shell-item-42".into(),
            name: "notes.txt".into(),
            original: "/home/alex/notes.txt".into(),
            deleted_unix: 1_783_252_800,
            folder: false,
        };
        let entry = entry(&item);
        assert!(entry.node_id.starts_with("recyclebin://notes.txt?id="));
        assert!(!entry.node_id.contains(&item.id));
        let encoded = entry.node_id.split_once("?id=").unwrap().1;
        assert_eq!(
            decode(encoded).unwrap().id,
            item.id
        );
        assert_eq!(entry.link_target.as_deref(), Some(item.original.as_str()));
    }

    #[test]
    fn internal_identity_projects_back_to_platform_metadata() {
        let metadata = RecycleBinItemMetadata {
            id: "shell-item-42".into(),
            name: "Folder".into(),
            original: "C:\\Users\\Alex\\Folder".into(),
            deleted_unix: 1_783_252_800,
            folder: true,
        };
        let identity = ItemIdentity::from(&metadata);
        assert_eq!(RecycleBinItemMetadata::from(identity), metadata);
    }
}
