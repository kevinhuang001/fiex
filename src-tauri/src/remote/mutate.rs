//! Creating, renaming and removing provider-backed connection nodes.
//!
//! Connection mounts are virtual: Core owns the metadata/cache bookkeeping and
//! the provider owns the bytes. Each mutation goes through [`Mount`] (so the
//! protocol and capability checks are the same as any other `vfs.*` call), then
//! updates the local snapshot so the explorer reflects it without waiting for a
//! full re-list.

use std::fs;

use crate::plugins::resolvers::fs::{mount_for_path, FsEntry, WriteRequest};

use super::{
    config::{check_cache_path, connection_for_path, local_for, navigation_path, rel_string},
    files::child_path,
    sync::{in_tree, load_meta, save_meta, RemoteMeta, RemoteMetaFile},
};

fn node_id(meta: &RemoteMeta, rel: &str) -> String {
    meta.files
        .get(rel)
        .map(|file| file.node_id.clone())
        .filter(|node| !node.is_empty())
        .unwrap_or_else(|| rel.to_string())
}

/// Project a provider entry onto the stored snapshot fields.
fn remember(entry: &FsEntry, file: &mut RemoteMetaFile) {
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

fn move_cache(old: &std::path::Path, new: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = new.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if new.exists() {
        if new.is_dir() {
            fs::remove_dir_all(new).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(new).map_err(|e| e.to_string())?;
        }
    }
    fs::rename(old, new).map_err(|e| e.to_string())
}

/// New entries are folders: `vfs.*` has no create-empty-file operation.
pub async fn create(path: &str, name: &str) -> Result<String, String> {
    let (conn, relative) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    let _guard = super::sync::lock(&conn).await;
    let rel = rel_string(&relative);
    let mount = mount_for_path(path)?;
    let mut meta = load_meta(&conn);
    let parent = node_id(&meta, &rel);
    let created = Some(mount.mkdir(&parent, name)?);
    let created_name = created
        .as_ref()
        .map_or_else(|| name.to_string(), |entry| entry.name.clone());
    let child = child_path(&rel, &created_name);
    let mut file = RemoteMetaFile {
        is_dir: true,
        ..RemoteMetaFile::default()
    };
    if let Some(entry) = &created {
        remember(entry, &mut file);
    }
    meta.files.insert(child.clone(), file);
    meta.errors.remove(&child);
    save_meta(&conn, &meta)?;
    navigation_path(&conn, &child)
}

pub async fn rename(path: &str, name: &str) -> Result<String, String> {
    let (conn, relative) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    let _guard = super::sync::lock(&conn).await;
    let rel = rel_string(&relative);
    if rel.is_empty() {
        return Err("cannot rename the connection root".into());
    }
    let mount = mount_for_path(path)?;
    let mut meta = load_meta(&conn);
    let node = node_id(&meta, &rel);
    let renamed = mount.rename(&node, name, None)?;
    let parent = rel.rsplit_once('/').map_or("", |(parent, _)| parent);
    let next = child_path(parent, &renamed.name);
    if next == rel {
        if let Some(file) = meta.files.get_mut(&rel) {
            remember(&renamed, file);
        }
        save_meta(&conn, &meta)?;
        return navigation_path(&conn, &next);
    }
    let old_local = local_for(&conn, &rel);
    let new_local = local_for(&conn, &next);
    if old_local.exists() {
        check_cache_path(&conn, &old_local)?;
        move_cache(&old_local, &new_local)?;
    }
    let mut file = meta.files.remove(&rel).unwrap_or_default();
    remember(&renamed, &mut file);
    if !file.is_dir && !new_local.is_file() {
        file.downloaded = false;
        file.uploaded = false;
        file.baseline.clear();
        file.local_stamp.clear();
    }
    let is_dir = file.is_dir;
    meta.files.insert(next.clone(), file);
    if is_dir {
        let prefix = format!("{rel}/");
        let keys: Vec<String> = meta
            .files
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .cloned()
            .collect();
        for key in keys {
            if let Some(value) = meta.files.remove(&key) {
                meta.files
                    .insert(format!("{next}/{}", &key[prefix.len()..]), value);
            }
        }
        let listed: Vec<String> = meta
            .listed
            .iter()
            .filter(|key| in_tree(key, &rel))
            .cloned()
            .collect();
        for key in listed {
            meta.listed.remove(&key);
            meta.listed.insert(format!("{next}{}", &key[rel.len()..]));
        }
    }
    meta.errors.remove(&rel);
    save_meta(&conn, &meta)?;
    navigation_path(&conn, &next)
}

/// Create an empty file through the provider's write stream. The provider
/// receives `parentId` and `name`, so a server that only supports overwriting
/// an existing node rejects this with its own error; Core does not guess.
pub async fn create_file(path: &str, name: &str) -> Result<String, String> {
    let (conn, relative) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    let _guard = super::sync::lock(&conn).await;
    let rel = rel_string(&relative);
    let mount = mount_for_path(path)?;
    let mut meta = load_meta(&conn);
    let parent = node_id(&meta, &rel);
    let created = mount
        .begin_write(WriteRequest {
            node_id: None,
            parent_id: Some(&parent),
            name: Some(name),
            size: Some(0),
            if_revision: None,
        })?
        .commit()?;
    let child = child_path(&rel, name);
    let mut file = RemoteMetaFile::default();
    if let Some(entry) = &created {
        remember(entry, &mut file);
    }
    if let Some(entry) = created {
        file.is_dir = false;
        file.node_id = entry.node_id;
    }
    meta.files.insert(child.clone(), file);
    meta.errors.remove(&child);
    save_meta(&conn, &meta)?;
    navigation_path(&conn, &child)
}

pub async fn remove(path: &str) -> Result<(), String> {
    let (conn, relative) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    let _guard = super::sync::lock(&conn).await;
    let rel = rel_string(&relative);
    if rel.is_empty() {
        return Err("cannot remove the connection root".into());
    }
    let mount = mount_for_path(path)?;
    let mut meta = load_meta(&conn);
    let node = node_id(&meta, &rel);
    mount.remove(&node)?;
    meta.files.retain(|key, _| !in_tree(key, &rel));
    meta.listed.retain(|key| !in_tree(key, &rel));
    meta.errors.retain(|key, _| !in_tree(key, &rel));
    let local = local_for(&conn, &rel);
    if local.exists() {
        check_cache_path(&conn, &local)?;
        if local.is_dir() {
            fs::remove_dir_all(&local).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&local).map_err(|e| e.to_string())?;
        }
    }
    save_meta(&conn, &meta)?;
    Ok(())
}
