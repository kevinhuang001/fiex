//! Bridges plugin mount streams to the Core filesystem stream handles.
use super::super::super::CHUNK;
use super::super::{FsError, FsErrorKind, ResourceBroker};
use super::{parent, plugin_node, remember_entry};
use fiex_provider_contracts::{FsEntry, FsRead, FsWrite};
use std::{
    io::Read,
    sync::{Arc, Mutex},
};

pub fn stream_read(resource: &ResourceBroker) -> Result<FsRead, FsError> {
    let (mount, node) = plugin_node(resource)?;
    let stream = mount.open_read(&node).map_err(FsError::from)?;
    let size = stream
        .size()
        .or_else(|| mount.stat(&node).ok().and_then(|entry| entry.size))
        .ok_or_else(|| FsError::new(FsErrorKind::Unsupported, "stream size is unknown"))?;
    Ok(FsRead::new(PluginReader { stream, offset: 0 }, Some(size)))
}

struct PluginReader {
    stream: crate::plugins::providers::fs::FsStream,
    offset: u64,
}

impl Read for PluginReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let (bytes, _) = self
            .stream
            .read_at(self.offset, output.len().min(CHUNK))
            .map_err(std::io::Error::other)?;
        output[..bytes.len()].copy_from_slice(&bytes);
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("read offset overflow"))?;
        Ok(bytes.len())
    }
}

pub fn stream_write(
    resource: &ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<FsWrite, FsError> {
    let stream = if create {
        let (mount, parent, name) = parent(resource)?;
        mount
            .begin_write(crate::plugins::providers::fs::WriteRequest {
                node_id: None,
                parent_id: Some(&parent),
                name: Some(&name),
                size,
                if_revision: None,
            })
            .map_err(FsError::from)?
    } else {
        let (mount, node) = plugin_node(resource)?;
        let entry = mount.stat(&node).map_err(FsError::from)?;
        mount
            .begin_write(crate::plugins::providers::fs::WriteRequest {
                node_id: Some(&node),
                parent_id: None,
                name: None,
                size,
                if_revision: entry.revision.as_deref(),
            })
            .map_err(FsError::from)?
    };
    let stream = Arc::new(Mutex::new(Some(stream)));
    let writer = PluginWriter(Arc::clone(&stream));
    let finish = Arc::clone(&stream);
    let path = resource.path.clone();
    Ok(FsWrite::with_commit(writer, move || {
        let entry = finish
            .lock()
            .map_err(|_| {
                fiex_provider_contracts::ProviderError::internal("write stream unavailable")
            })?
            .take()
            .ok_or_else(|| fiex_provider_contracts::ProviderError::internal("write stream closed"))?
            .commit()
            .map_err(|error| {
                fiex_provider_contracts::ProviderError::internal(FsError::from(error).to_string())
            })?;
        if let Some(entry) = entry {
            remember_entry(&path, &entry).map_err(|error| {
                fiex_provider_contracts::ProviderError::internal(error.to_string())
            })?;
            Ok(entry)
        } else {
            Ok(FsEntry::default())
        }
    }))
}

struct PluginWriter(Arc<Mutex<Option<crate::plugins::providers::fs::FsWriteStream>>>);

impl std::io::Write for PluginWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let mut stream = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("write stream unavailable"))?;
        let stream = stream
            .as_mut()
            .ok_or_else(|| std::io::Error::other("write stream closed"))?;
        stream.write(bytes).map_err(std::io::Error::other)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
