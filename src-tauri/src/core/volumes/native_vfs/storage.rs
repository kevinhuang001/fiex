//! Platform storage implementations projected through the shared VFS DTOs.
use crate::core::files::Entry;
use fiex_provider_contracts::FsOperation;

pub(super) fn entry(value: serde_json::Value) -> Result<Entry, String> {
    crate::core::fs::presentation::entry(value)
}

pub(super) fn decorated_entry(path: &str, mut entry: Entry) -> Entry {
    let capabilities = crate::plugins::resource::file_capabilities::for_entry(path, &entry);
    entry.allowed_ops = [
        FsOperation::List,
        FsOperation::Stat,
        FsOperation::Search,
        FsOperation::Read,
        FsOperation::Write,
        FsOperation::Mkdir,
        FsOperation::Rename,
        FsOperation::Trash,
        FsOperation::Remove,
    ]
    .into_iter()
    .filter(|operation| capabilities.allows(*operation))
    .map(|operation| operation.as_str().to_owned())
    .collect();
    entry
}

pub(super) fn root_entry(id: &str, path: &str, operations: &[FsOperation]) -> Entry {
    Entry {
        path: path.into(),
        name: id.into(),
        is_dir: true,
        allowed_ops: operations
            .iter()
            .map(|operation| operation.as_str().to_owned())
            .collect(),
        ..Default::default()
    }
}
