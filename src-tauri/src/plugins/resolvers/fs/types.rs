use super::*;

/// Parameters for a conditional write.
pub struct WriteRequest<'a> {
    pub node_id: Option<&'a str>,
    pub parent_id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub size: Option<u64>,
    pub if_revision: Option<&'a str>,
}

/// Kind of node as reported by a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsKind {
    File,
    Directory,
    Symlink,
}

/// One provider entry after validation. Paths never appear on the wire; Core
/// binds `node_id` to its own mount-relative location.
#[derive(Clone, Debug)]
pub struct FsEntry {
    pub node_id: String,
    pub name: String,
    pub kind: FsKind,
    pub size: Option<u64>,
    pub modified: Option<i64>,
    pub extension: Option<String>,
    pub mime: Option<String>,
    pub revision: Option<String>,
    pub readonly: Option<bool>,
    pub link_target: Option<String>,
    pub allowed_ops: Option<Vec<FsOperation>>,
    pub relative_path: Option<String>,
}

impl FsEntry {
    pub fn parse(value: Value) -> Result<Self, String> {
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
        if value
            .get("readonly")
            .is_some_and(|value| !value.is_null() && !value.is_boolean())
        {
            return Err("invalid readonly flag".into());
        }
        Ok(Self {
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
            readonly: value.get("readonly").and_then(Value::as_bool),
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

    pub fn is_dir(&self) -> bool {
        self.kind == FsKind::Directory
    }

    /// The node allows `operation` when it did not narrow the provider set.
    pub fn allows(&self, operation: FsOperation) -> bool {
        self.allowed_ops
            .as_ref()
            .is_none_or(|ops| ops.contains(&operation))
    }

    pub fn is_readonly(&self) -> bool {
        self.readonly.unwrap_or(false)
            || !(self.allows(FsOperation::Write)
                || self.allows(FsOperation::Mkdir)
                || self.allows(FsOperation::Rename)
                || self.allows(FsOperation::Remove))
    }
}

/// One validated listing page collection.
#[derive(Default)]
pub struct FsListing {
    pub entries: Vec<FsEntry>,
    pub warnings: Vec<String>,
    pub truncated: bool,
}
