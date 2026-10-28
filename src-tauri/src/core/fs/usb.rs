//! Raw-driver resources use the same bounded read handles as other storage.
use super::*;
use crate::usbfs::{self, Route};
use std::io::{Seek, Write};
pub(super) struct UsbFsAdapter {
    route: Route,
}
impl UsbFsAdapter {
    pub fn new(route: Route) -> Self {
        Self { route }
    }
}
impl FsAdapter for UsbFsAdapter {
    fn stat(&self) -> Result<Value, String> {
        serde_json::to_value(usbfs::entry(&self.route)?).map_err(|e| e.to_string())
    }
    fn list(&self) -> Result<Value, String> {
        serde_json::to_value(usbfs::listing(&self.route)?).map_err(|e| e.to_string())
    }
    fn require(&self, op: FsOperation, _create: bool) -> Result<(), String> {
        usbfs::ensure_access(&self.route)?;
        if !_create {
            let entry = usbfs::entry(&self.route)?;
            if (op == FsOperation::Read && entry.is_dir)
                || (matches!(
                    op,
                    FsOperation::List | FsOperation::Search | FsOperation::Mkdir
                ) && !entry.is_dir)
            {
                return Err("Unsupported: operation does not apply to node kind".into());
            }
        }
        if matches!(
            op,
            FsOperation::List | FsOperation::Stat | FsOperation::Read | FsOperation::Search
        ) {
            return Ok(());
        }
        if matches!(
            self.route.kind,
            usbfs::FsKind::Apfs | usbfs::FsKind::Hfs | usbfs::FsKind::Btrfs
        ) {
            return Err("Unsupported: read-only USB filesystem".into());
        }
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.route.driver_device)
            .map_err(|_| "PermissionDenied: OS has not granted raw-device writes")?;
        if self.route.rel.is_empty()
            && matches!(
                op,
                FsOperation::Remove | FsOperation::Rename | FsOperation::Write
            )
        {
            return Err("cannot modify USB root".into());
        }
        Ok(())
    }
    fn open_read(&self) -> Result<Box<dyn ReadHandle>, String> {
        let entry = usbfs::entry(&self.route)?;
        if entry.is_dir {
            return Err("expected a file".into());
        }
        let device = std::fs::File::open(&self.route.driver_device).map_err(|e| e.to_string())?;
        let stamp = device.metadata().map_err(|e| e.to_string())?;
        Ok(Box::new(UsbRead {
            route: self.route.clone(),
            size: entry.size,
            stamp,
            _device: device,
        }))
    }
    fn begin_write(&self, create: bool) -> Result<Box<dyn WriteHandle>, String> {
        self.require(FsOperation::Write, create)?;
        let staged = tempfile::tempfile().map_err(|e| e.to_string())?;
        Ok(Box::new(UsbWrite {
            route: self.route.clone(),
            staged,
            replace: !create,
            generation: usbfs::generation(&self.route)?,
        }))
    }
    fn mkdir(&self) -> Result<Value, String> {
        self.require(FsOperation::Mkdir, true)?;
        let (parent, name) = self
            .route
            .rel
            .rsplit_once('/')
            .unwrap_or(("", &self.route.rel));
        if name.is_empty() {
            return Err("cannot create USB root".into());
        }
        usbfs::create(
            &Route {
                rel: parent.into(),
                ..self.route.clone()
            },
            name,
            true,
        )?;
        self.stat()
    }
    fn remove(&self) -> Result<(), String> {
        self.require(FsOperation::Remove, false)?;
        remove_tree(&self.route, 0)
    }
    fn rename(&self, destination: &ResourceBroker) -> Result<(), String> {
        self.require(FsOperation::Rename, false)?;
        let other = usbfs::route(&destination.path).ok_or("invalid USB destination")?;
        let (parent, name) = other.rel.rsplit_once('/').unwrap_or(("", &other.rel));
        let source_parent = self
            .route
            .rel
            .rsplit_once('/')
            .map(|(p, _)| p)
            .unwrap_or("");
        if other.root != self.route.root || parent != source_parent {
            return Err("Unsupported: cross-directory USB rename; use TransferEngine".into());
        }
        if usbfs::entry(&other).is_ok() {
            return Err("destination already exists".into());
        }
        usbfs::rename(&self.route, name)
    }
}
fn remove_tree(route: &Route, depth: usize) -> Result<(), String> {
    if depth > 128 {
        return Err("USB tree limit exceeded".into());
    }
    if usbfs::entry(route)?.is_dir {
        for child in usbfs::listing(route)? {
            let next = usbfs::route(&child.path).ok_or("invalid USB child")?;
            if next.root != route.root {
                return Err("USB child escaped mount".into());
            }
            remove_tree(&next, depth + 1)?;
        }
    }
    usbfs::remove(route)
}
struct UsbWrite {
    route: Route,
    staged: std::fs::File,
    replace: bool,
    generation: String,
}
impl WriteHandle for UsbWrite {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if usbfs::generation(&self.route)? != self.generation {
            return Err("StaleRef: USB device replaced".into());
        }
        self.staged.write_all(bytes).map_err(|e| e.to_string())
    }
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String> {
        let position = match kind {
            "start" => std::io::SeekFrom::Start(
                u64::try_from(offset).map_err(|_| "negative write offset")?,
            ),
            "current" => std::io::SeekFrom::Current(offset),
            "end" => std::io::SeekFrom::End(offset),
            _ => return Err("invalid seek kind".into()),
        };
        self.staged.seek(position).map_err(|e| e.to_string())
    }
    fn commit(mut self: Box<Self>) -> Result<Value, String> {
        if usbfs::generation(&self.route)? != self.generation {
            return Err("StaleRef: USB device replaced".into());
        }
        usbfs::ensure_access(&self.route)?;
        self.staged.sync_all().map_err(|e| e.to_string())?;
        usbfs::commit_staged(&self.route, &mut self.staged, self.replace)?;
        Ok(Value::Null)
    }
}
struct UsbRead {
    route: Route,
    size: u64,
    stamp: std::fs::Metadata,
    _device: std::fs::File,
}
impl ReadHandle for UsbRead {
    fn size(&self) -> u64 {
        self.size
    }
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        let current = std::fs::metadata(&self.route.driver_device)
            .map_err(|_| "StaleRef: USB device disconnected")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if current.dev() != self.stamp.dev()
                || current.ino() != self.stamp.ino()
                || current.rdev() != self.stamp.rdev()
            {
                return Err("StaleRef: USB device replaced".into());
            }
        }
        #[cfg(not(unix))]
        if current.len() != self.stamp.len() || current.created().ok() != self.stamp.created().ok()
        {
            return Err("StaleRef: USB device replaced".into());
        }
        let mut bytes = vec![
            0;
            length.min(CHUNK).min(
                self.size
                    .saturating_sub(offset)
                    .try_into()
                    .unwrap_or(usize::MAX)
            )
        ];
        let size = usbfs::read_at(&self.route, offset, &mut bytes)?;
        bytes.truncate(size);
        Ok((bytes, offset.saturating_add(size as u64) >= self.size))
    }
}
