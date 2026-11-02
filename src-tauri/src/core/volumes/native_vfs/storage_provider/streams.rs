//! Scoped local and USB read/write stream implementations.
use super::{provider_error, shared_entry, storage};
use crate::{
    core::fs::ResourceBroker,
    plugins::resource::fs_access,
    usbfs::{self, Route},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use fiex_provider_contracts::{FsRead, FsWrite, ProviderError};
use std::{
    io::{self, Read, Write},
    sync::{Arc, Mutex},
};

pub(super) fn read(resource: ResourceBroker) -> Result<FsRead, ProviderError> {
    if let Some(route) = usbfs::route(&resource.path) {
        let entry = usbfs::entry(&route).map_err(provider_error)?;
        if entry.is_dir {
            return Err(ProviderError::invalid_params(
                "Unsupported: expected a file",
            ));
        }
        let device = std::fs::File::open(&route.driver_device).map_err(provider_error)?;
        let generation = usbfs::generation(&route).map_err(provider_error)?;
        let state = Arc::new(UsbReadState {
            route,
            _device: device,
            generation,
            size: entry.size,
        });
        let read_state = Arc::clone(&state);
        return Ok(FsRead::with_read_at(
            UsbReader { state, offset: 0 },
            Some(entry.size),
            move |offset, length| read_state.read_at(offset, length).map_err(provider_error),
        ));
    }
    let value = fs_access::open_read(&resource.owner, &resource.scope, &resource.path)
        .map_err(provider_error)?;
    let handle = value["handle"]
        .as_str()
        .ok_or_else(|| ProviderError::internal("missing read handle"))?
        .to_owned();
    let size = value["size"]
        .as_u64()
        .ok_or_else(|| ProviderError::internal("missing read size"))?;
    let state = Arc::new(LocalReadState {
        owner: resource.owner,
        handle,
    });
    let read_state = Arc::clone(&state);
    Ok(FsRead::with_read_at(
        LocalReader { state, offset: 0 },
        Some(size),
        move |offset, length| read_state.read_at(offset, length).map_err(provider_error),
    ))
}

pub(super) fn write(resource: ResourceBroker, create: bool) -> Result<FsWrite, ProviderError> {
    if usbfs::route(&resource.path).is_some() {
        return Err(ProviderError::invalid_params(
            "Unsupported: USB filesystem fallback is read-only",
        ));
    }
    let handle = fs_access::begin_write(&resource.owner, &resource.scope, &resource.path, create)
        .map_err(provider_error)?;
    let staged = Arc::new(Mutex::new(Some(LocalWrite {
        owner: resource.owner.clone(),
        handle,
        finished: false,
    })));
    let writer = LocalStage(Arc::clone(&staged));
    let commit_stage = Arc::clone(&staged);
    Ok(FsWrite::with_commit(writer, move || {
        let mut write = commit_stage
            .lock()
            .map_err(|_| ProviderError::internal("write stream unavailable"))?
            .take()
            .ok_or_else(|| ProviderError::internal("write stream closed"))?;
        write.commit().map_err(provider_error)?;
        let value = fs_access::stat(&resource.scope, &resource.path).map_err(provider_error)?;
        let entry = storage::entry(value).map_err(provider_error)?;
        Ok(shared_entry(storage::decorated_entry(
            &resource.path,
            entry,
        )))
    }))
}

struct LocalReadState {
    owner: String,
    handle: String,
}

impl LocalReadState {
    fn read_at(&self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        let value = fs_access::read_open(
            &self.owner,
            &self.handle,
            offset,
            length.min(crate::core::fs::CHUNK),
            None,
        )?;
        let bytes = STANDARD
            .decode(value["base64"].as_str().ok_or("missing read data")?)
            .map_err(|error| error.to_string())?;
        Ok((bytes, value["eof"] == true))
    }
}

impl Drop for LocalReadState {
    fn drop(&mut self) {
        let _ = fs_access::close_read(&self.owner, &self.handle);
    }
}

struct LocalReader {
    state: Arc<LocalReadState>,
    offset: u64,
}

impl Read for LocalReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let (data, _) = self
            .state
            .read_at(self.offset, output.len())
            .map_err(io::Error::other)?;
        output[..data.len()].copy_from_slice(&data);
        self.offset = self
            .offset
            .checked_add(data.len() as u64)
            .ok_or_else(|| io::Error::other("read offset overflow"))?;
        Ok(data.len())
    }
}

struct UsbReadState {
    route: Route,
    _device: std::fs::File,
    generation: String,
    size: u64,
}

impl UsbReadState {
    fn read_at(&self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        if usbfs::generation(&self.route)? != self.generation {
            return Err("StaleRef: USB device replaced or ejected".into());
        }
        let mut bytes = vec![
            0;
            length.min(crate::core::fs::CHUNK).min(
                usize::try_from(self.size.saturating_sub(offset)).unwrap_or(usize::MAX),
            )
        ];
        let read = usbfs::read_at(&self.route, offset, &mut bytes)?;
        bytes.truncate(read);
        Ok((bytes, offset.saturating_add(read as u64) >= self.size))
    }
}

struct UsbReader {
    state: Arc<UsbReadState>,
    offset: u64,
}

impl Read for UsbReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let (bytes, _) = self
            .state
            .read_at(self.offset, output.len())
            .map_err(io::Error::other)?;
        output[..bytes.len()].copy_from_slice(&bytes);
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("read offset overflow"))?;
        Ok(bytes.len())
    }
}

struct LocalWrite {
    owner: String,
    handle: String,
    finished: bool,
}

impl LocalWrite {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        fs_access::write_chunk(&self.owner, &self.handle, &STANDARD.encode(bytes))
    }

    fn commit(&mut self) -> Result<(), String> {
        fs_access::commit(&self.owner, &self.handle)?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for LocalWrite {
    fn drop(&mut self) {
        if !self.finished {
            let _ = fs_access::abort(&self.owner, &self.handle);
        }
    }
}

struct LocalStage(Arc<Mutex<Option<LocalWrite>>>);

impl Write for LocalStage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("write stream unavailable"))?
            .as_mut()
            .ok_or_else(|| io::Error::other("write stream closed"))?
            .write(bytes)
            .map_err(io::Error::other)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
