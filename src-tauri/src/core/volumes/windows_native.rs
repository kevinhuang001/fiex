//! Native volume enumeration, drive-letter assignment, and safe media eject.

mod discovery;
mod mount;
mod shell;

pub(crate) use discovery::enumerate_unmounted_removable;
pub(crate) use mount::{eject_drive, mount_volume};

#[derive(Clone, Debug)]
pub(crate) struct RemovableVolume {
    pub device: String,
    pub name: String,
    pub filesystem: String,
    pub total: u64,
    pub available: u64,
}
