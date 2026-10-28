//! Desktop projections share the broker authorization and adapter boundary.
use super::*;
use crate::core::files::{Entry, Listing};
use std::path::Path;

pub(super) fn entry(value: Value) -> Result<Entry, String> {
    let mut fields = serde_json::to_value(Entry::default()).map_err(|e| e.to_string())?;
    let object = value.as_object().ok_or("invalid adapter entry")?;
    for (key, value) in object {
        if !value.is_null() {
            fields[key] = value.clone();
        }
    }
    if let Some(symlink) = object.get("symlink") {
        fields["isSymlink"] = symlink.clone();
    }
    serde_json::from_value(fields).map_err(|e| e.to_string())
}
pub(super) fn listing<A: FsAdapter + ?Sized>(
    adapter: &A,
    path: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<Listing, String> {
    let values = adapter.list()?;
    let filter = crate::core::ignores::IgnoreSet::new(ignores);
    let mut entries = Vec::new();
    for value in values.as_array().ok_or("invalid adapter listing")? {
        let item = entry(value.clone())?;
        if (hidden || !item.name.starts_with('.')) && !filter.is_ignored(Path::new(&item.name)) {
            entries.push(item);
        }
    }
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    Ok(Listing {
        path: path.into(),
        entries,
        read_only: false,
        can_create: adapter.require(FsOperation::Mkdir, false).is_ok(),
        virtual_volume: true,
        warnings: Vec::new(),
        default_view: None,
    })
}
impl ResourceBroker {
    pub fn desktop_entry(&self) -> Result<Entry, String> {
        self.scope.authorize_method("fs.stat")?;
        self.require(FsOperation::Stat, false)?;
        let mut entry = self.adapter()?.desktop_entry()?;
        entry.path = self.path.clone();
        Ok(entry)
    }
    pub fn desktop_listing(&self, hidden: bool, ignores: &[String]) -> Result<Listing, String> {
        self.scope.authorize_method("fs.list")?;
        self.require(FsOperation::List, false)?;
        self.adapter()?.desktop_listing(&self.path, hidden, ignores)
    }
}
