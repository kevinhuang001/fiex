//! Core projection adapter for the module-owned Windows Recycle Bin backend.
use crate::core::files::Entry;
use fiex_provider_contracts::{FsEntry, FsListing};

pub const URI: &str = fiex_trash::RECYCLE_BIN_URI;

pub fn is_virtual(path: &str) -> bool {
    fiex_trash::is_recycle_bin_uri(path)
}

fn project(entry: FsEntry) -> Entry {
    let revision = entry.revision.clone();
    let mut entry = crate::core::volumes::native_vfs::core_entry(entry);
    entry.sync = revision;
    entry
}

fn project_listing(listing: FsListing) -> Vec<Entry> {
    listing.entries.into_iter().map(project).collect()
}

pub fn list() -> Result<Vec<Entry>, String> {
    fiex_trash::system_recycle_bin().list().map(project_listing)
}
