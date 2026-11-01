//! Desktop projections share the broker authorization and Provider boundary.
use super::*;
use crate::core::files::{Entry, Listing};

pub(crate) fn entry(value: Value) -> Result<Entry, String> {
    let mut fields = serde_json::to_value(Entry::default()).map_err(|e| e.to_string())?;
    let object = value.as_object().ok_or("invalid VFS entry")?;
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
impl ResourceBroker {
    pub fn desktop_entry(&self) -> Result<Entry, FsError> {
        self.scope.authorize_method("host.fs.stat")?;
        self.require(FsOperation::Stat, false)?;
        let mut entry = provider_registry::desktop_entry(self)?;
        entry.path = self.path.clone();
        Ok(entry)
    }
    pub fn desktop_listing(&self, hidden: bool, ignores: &[String]) -> Result<Listing, FsError> {
        self.scope.authorize_method("host.fs.list")?;
        self.require(FsOperation::List, false)?;
        let listing = super::list_service::fresh(self)?;
        Ok(directory_cache::filtered(listing, hidden, ignores))
    }
}
