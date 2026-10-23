//! Shared-buffer registry for the plugin data plane.
use crate::plugin_sdk::ipc::{BufferDescriptor, SharedBuffer};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const MAX_BUFFER: usize = 64 * 1024 * 1024;
const MAX_BUFFERS_PER_PLUGIN: usize = 32;

struct Entry {
    owner: String,
    buffer: SharedBuffer,
}

fn registry() -> &'static Mutex<HashMap<String, Entry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn create(owner: &str, size: usize, writable: bool) -> Result<BufferDescriptor, String> {
    if size == 0 || size > MAX_BUFFER {
        return Err("shared buffer size is invalid".into());
    }
    let mut buffers = registry()
        .lock()
        .map_err(|_| "buffer registry lock poisoned")?;
    if buffers
        .values()
        .filter(|entry| entry.owner == owner)
        .count()
        >= MAX_BUFFERS_PER_PLUGIN
    {
        return Err("too many shared buffers".into());
    }
    let (buffer, descriptor) = SharedBuffer::create(size, writable)?;
    buffers.insert(
        descriptor.id.clone(),
        Entry {
            owner: owner.to_owned(),
            buffer,
        },
    );
    Ok(descriptor)
}

pub fn map(owner: &str, id: &str) -> Result<BufferDescriptor, String> {
    let buffers = registry()
        .lock()
        .map_err(|_| "buffer registry lock poisoned")?;
    let entry = buffers.get(id).ok_or("shared buffer not found")?;
    if entry.owner != owner {
        return Err("shared buffer belongs to another plugin".into());
    }
    Ok(entry.buffer.descriptor().clone())
}

pub fn release(owner: &str, id: &str) -> Result<(), String> {
    let mut buffers = registry()
        .lock()
        .map_err(|_| "buffer registry lock poisoned")?;
    let entry = buffers.get(id).ok_or("shared buffer not found")?;
    if entry.owner != owner {
        return Err("shared buffer belongs to another plugin".into());
    }
    buffers.remove(id);
    Ok(())
}
