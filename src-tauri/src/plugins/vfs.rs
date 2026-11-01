//! Generic plugin-owned virtual filesystems.
//!
//! Explicit VFS extensions are routed through Core-owned view identities and the
//! shared [`crate::plugins::resolvers::fs`], so views use the same `vfs.*` protocol as
//! connections. Views remain navigation identities; UI extensions never confer
//! provider eligibility.

use crate::core::files::{Entry, Listing};
use crate::plugins::provider_registry::FsOperation;
use crate::plugins::resolvers::fs::{FsEntry, FsKind, Mount, Source, WriteRequest};
use serde_json::Value;

mod exports;
pub use exports::*;

mod mutations;
pub use mutations::*;

#[cfg(test)]
mod tests;

/// Stream reads are chunked so a large export never sits in memory.
const READ_CHUNK: usize = 256 * 1024;

/// Claim every Core URI, including invalid/revoked ones, before OS fallback.
pub fn handles(path: &str) -> bool {
    crate::plugins::mounts::handles(path)
}

fn resolve(
    path: &str,
) -> Result<(crate::plugins::mounts::Uri, crate::plugins::mounts::View), String> {
    let (uri, source) = crate::plugins::mounts::resolve(path)?;
    match source {
        crate::plugins::mounts::Source::View(view) => Ok((uri, view)),
        crate::plugins::mounts::Source::Connection(_) => {
            Err("connection request requires the Core connection adapter".into())
        }
    }
}

fn node(
    path: &str,
) -> Result<
    (
        crate::plugins::mounts::Uri,
        crate::plugins::mounts::View,
        String,
    ),
    String,
> {
    let (uri, mut view) = resolve(path)?;
    if view.node_id(&uri.relative)?.is_none() {
        // Address-bar/bookmark navigation may target an opaque node not visited
        // this session. Resolve only its ancestors, never enumerate the mount.
        let mut parent = String::new();
        for part in uri.relative.split('/') {
            let target = if parent.is_empty() {
                part.to_string()
            } else {
                format!("{parent}/{part}")
            };
            let current = crate::plugins::mounts::view(&uri.mount_id)?;
            if current.node_id(&target)?.is_none() {
                list(
                    &crate::plugins::mounts::uri(&uri.mount_id, &parent)?,
                    true,
                    &[],
                )?;
            }
            parent = target;
        }
        view = crate::plugins::mounts::view(&uri.mount_id)?;
    }
    let id = view.node_id(&uri.relative)?.ok_or("VFS node not found")?;
    Ok((uri, view, id))
}

/// The provider identity a view sends with every `vfs.*` request.
fn mount_for(uri: &crate::plugins::mounts::Uri, view: &crate::plugins::mounts::View) -> Mount {
    Mount {
        mount_id: uri.mount_id.clone(),
        plugin_id: view.descriptor.plugin_id.clone(),
        source: Source::View {
            contribution_id: view.descriptor.contribution_id.clone(),
            view_context: view.descriptor.view_context.clone(),
        },
        scope: Some(view.scope.clone()),
    }
}

/// Child location of `name`, never trusting a provider-supplied path.
fn child(relative: &str, name: &str) -> Result<String, String> {
    crate::plugins::mounts::segment(name)?;
    Ok(if relative.is_empty() {
        name.to_string()
    } else {
        format!("{relative}/{name}")
    })
}

/// Intersect the Core-verified mount ceiling with the node's restrictions.
/// This stays pure so tests cannot accidentally grant a fake mount authority.
fn allowed_operations(
    entry: &FsEntry,
    provider_supports: impl Fn(FsOperation) -> bool,
) -> Vec<String> {
    FsOperation::ALL
        .into_iter()
        .filter(|operation| provider_supports(*operation) && entry.allows(*operation))
        .map(|operation| operation.as_str().to_string())
        .collect()
}

fn project_entry(
    mount: &Mount,
    uri: &crate::plugins::mounts::Uri,
    relative: &str,
    entry: FsEntry,
) -> Result<(Entry, String), String> {
    crate::plugins::mounts::segment(&entry.name)?;
    // The provider's node is authoritative, but its name must match the
    // location Core already bound it to.
    if !relative.is_empty() && relative.rsplit('/').next() != Some(entry.name.as_str()) {
        return Err("provider node name does not match its location".into());
    }
    let path = crate::plugins::mounts::uri(&uri.mount_id, relative)?;
    let allowed_ops = allowed_operations(&entry, |operation| mount.supports(operation));
    // Read-only is an explicit provider flag. Per-operation restrictions live
    // in `allowed_ops`: a provider that supports only `remove` must still be
    // able to delete, so the absence of the other mutating operations cannot
    // make the whole node read-only.
    let readonly = entry.readonly.unwrap_or(false);
    Ok((
        Entry {
            path,
            name: entry.name,
            is_dir: entry.kind == FsKind::Directory,
            is_symlink: entry.kind == FsKind::Symlink,
            size: entry.size.unwrap_or(0),
            modified: entry.modified.unwrap_or(0).max(0) as u64,
            created: 0,
            extension: entry.extension.unwrap_or_default(),
            mime: entry.mime.unwrap_or_default(),
            readonly,
            link_target: entry.link_target,
            tags: Vec::new(),
            has_cache: false,
            pinned: false,
            uploaded: false,
            sync: None,
            allowed_ops,
        },
        entry.node_id,
    ))
}

fn bound_entry(
    mount: &Mount,
    uri: &crate::plugins::mounts::Uri,
    relative: &str,
    entry: FsEntry,
) -> Result<Entry, String> {
    let (entry, id) = project_entry(mount, uri, relative, entry)?;
    crate::plugins::mounts::bind_node(&uri.mount_id, relative, &id)?;
    Ok(entry)
}

pub fn entry_from(value: Value) -> Result<Entry, String> {
    serde_json::from_value(value).map_err(|error| {
        crate::i18n::format(
            "插件返回了无效的目录项：{error}",
            &[("error", format!("{}", error))],
        )
    })
}

pub fn list(path: &str, hidden: bool, ignores: &[String]) -> Result<Listing, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let listing = mount.list(&node_id, hidden, ignores)?;
    let mut entries = Vec::with_capacity(listing.entries.len());
    for item in listing.entries {
        let relative = child(&uri.relative, &item.name)?;
        entries.push(bound_entry(&mount, &uri, &relative, item)?);
    }
    // Creating a folder needs the provider's `mkdir` and the directory node's
    // own `allowedOps`/`readonly`. The toolbar reads this flag to enable its
    // single new-folder affordance, and `create_entry` re-checks the same rule,
    // so the button can never offer an operation this view cannot perform.
    let can_create = crate::plugins::resource::file_capabilities::for_path(path).create;
    Ok(Listing {
        // A virtual volume is writable when the provider implements at least
        // one mutating operation; each entry narrows that through allowedOps.
        read_only: false,
        can_create,
        virtual_volume: true,
        path: crate::plugins::mounts::uri(&uri.mount_id, &uri.relative)?,
        entries,
        warnings: listing.warnings,
        default_view: None,
    })
}

pub fn stat(path: &str) -> Result<Entry, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let entry = mount.stat(&node_id)?;
    bound_entry(&mount, &uri, &uri.relative, entry)
}

pub(crate) fn adapter_node(path: &str) -> Result<(Mount, String), String> {
    let (uri, view, node_id) = node(path)?;
    Ok((mount_for(&uri, &view), node_id))
}

/// Read at most `max_bytes` from the start of a file through a provider stream.
pub fn read_prefix(path: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let stream = mount.open_read(&node_id)?;
    let mut bytes = Vec::new();
    let mut offset = 0u64;
    while bytes.len() < max_bytes {
        let want = (max_bytes - bytes.len()).min(READ_CHUNK);
        let (chunk, eof) = stream.read_at(offset, want)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);
        if eof {
            break;
        }
    }
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn search_stream(
    path: &str,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(Entry),
) -> Result<bool, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    mount.search_stream(
        &node_id,
        query,
        hidden,
        ignores,
        limit,
        cancelled,
        &mut |item| {
            let relative = item
                .relative_path
                .clone()
                .ok_or("search entry missing relativePath")?;
            crate::plugins::mounts::uri(&uri.mount_id, &relative)?;
            if !uri.relative.is_empty() && !relative.starts_with(&format!("{}/", uri.relative)) {
                return Err("search entry escaped subtree".into());
            }
            emit(bound_entry(&mount, &uri, &relative, item)?);
            Ok(())
        },
    )
}
