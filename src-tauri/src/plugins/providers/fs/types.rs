use super::*;
use fiex_provider_contracts::FsOperation;

/// Parameters for a conditional write.
pub struct WriteRequest<'a> {
    pub node_id: Option<&'a str>,
    pub parent_id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub size: Option<u64>,
    pub if_revision: Option<&'a str>,
}

pub use fiex_provider_contracts::{FsEntry, FsKind, FsListing};

/// Validate an untrusted provider entry before Core binds its opaque node ID
/// to a mount-relative path. The value type itself is shared with the SDK.
pub fn parse_entry(value: Value) -> Result<FsEntry, String> {
    let node_id = value
        .get("nodeId")
        .and_then(Value::as_str)
        .ok_or("provider entry is missing nodeId")?
        .to_string();
    if node_id.len() > 16_384 || node_id.contains('\0') {
        return Err("provider returned an invalid node id".into());
    }
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .ok_or("provider entry is missing name")?
        .to_string();
    crate::plugins::mounts::segment(&name)?;
    let kind = match value.get("kind").and_then(Value::as_str) {
        Some("directory") => FsKind::Directory,
        Some("symlink") => FsKind::Symlink,
        Some("file") | None => FsKind::File,
        Some(_) => return Err("provider returned an unknown node kind".into()),
    };
    let allowed_ops = value
        .get("allowedOps")
        .filter(|value| !value.is_null())
        .map(|value| {
            let values = value.as_array().ok_or("invalid allowedOps")?;
            values
                .iter()
                .map(|value| {
                    let name = value.as_str().ok_or("invalid allowedOps entry")?;
                    Ok(FsOperation::parse(name))
                })
                .collect::<Result<Vec<_>, String>>()
                .map(|operations| operations.into_iter().flatten().collect())
        })
        .transpose()?;
    if value.get("readonly").is_some() {
        return Err("readonly is removed; declare supported operations with allowedOps".into());
    }
    Ok(FsEntry {
        node_id,
        name,
        kind,
        size: value.get("size").and_then(Value::as_u64),
        modified: value.get("modified").and_then(Value::as_i64),
        extension: value
            .get("extension")
            .and_then(Value::as_str)
            .map(str::to_owned),
        mime: value.get("mime").and_then(Value::as_str).map(str::to_owned),
        revision: value
            .get("revision")
            .and_then(Value::as_str)
            .map(str::to_owned),
        link_target: value
            .get("linkTarget")
            .and_then(Value::as_str)
            .map(str::to_owned),
        allowed_ops,
        relative_path: value
            .get("relativePath")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}
