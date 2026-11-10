//! Read only Finder Trash metadata from a `.DS_Store` B-tree.
//!
//! The general-purpose `ds_store` crate rejects record keys it does not know.
//! Finder writes many unrelated keys beside `ptbL` and `ptbN`, so this reader
//! skips those records while consuming their typed values safely.
//!
//! The bounded B-tree reader is adapted from `trash-core::macos` (Apache-2.0),
//! with Fiex-specific lookup and path validation. See `tests/data/LICENSE-APACHE`.
use std::collections::HashSet;

const HEADER_LEN: usize = 36;

pub(super) fn put_back(data: &[u8], trash_name: &str) -> Option<(String, String)> {
    if data.len() < HEADER_LEN {
        return None;
    }
    let mut header = Cursor::new(data);
    if header.u32()? != 1 || header.take(4)? != b"Bud1" {
        return None;
    }
    let root_offset = header.u32()?;
    let root_size = header.u32()?;
    if header.u32()? != root_offset {
        return None;
    }

    let root = block(data, root_offset, root_size)?;
    let mut root = Cursor::new(root);
    let offset_count = usize::try_from(root.u32()?).ok()?;
    root.u32()?;
    if offset_count > root.remaining() / 4 {
        return None;
    }
    let padded_count = offset_count.div_ceil(256).checked_mul(256)?;
    let mut offsets = Vec::with_capacity(offset_count);
    for index in 0..padded_count {
        let offset = root.u32()?;
        if index < offset_count {
            offsets.push(offset);
        }
    }

    let toc_count = root.u32()?;
    let mut dsdb = None;
    for _ in 0..toc_count {
        let key_len = usize::from(root.u8()?);
        let key = root.take(key_len)?;
        let block_id = root.u32()?;
        if key == b"DSDB" {
            dsdb = Some(block_id);
        }
    }

    let master = block_by_id(data, &offsets, dsdb?)?;
    let mut master = Cursor::new(master);
    let root_node = master.u32()?;
    master.u32()?;
    master.u32()?;
    let node_count = usize::try_from(master.u32()?).ok()?;

    let mut pending = vec![root_node];
    let mut visited = HashSet::new();
    let mut original_name = None;
    let mut original_location = None;
    while let Some(node_id) = pending.pop() {
        if !visited.insert(node_id) {
            continue;
        }
        if visited.len() > node_count.saturating_add(1) {
            return None;
        }
        let node = block_by_id(data, &offsets, node_id)?;
        let mut node = Cursor::new(node);
        let next_node = node.u32()?;
        let record_count = node.u32()?;
        for _ in 0..record_count {
            if next_node != 0 {
                pending.push(node.u32()?);
            }
            let record = read_record(&mut node)?;
            if record.name == trash_name {
                if record.key == *b"ptbN" {
                    original_name = record.string_value;
                } else if record.key == *b"ptbL" {
                    original_location = record.string_value;
                }
            }
            if let (Some(name), Some(location)) =
                (original_name.as_deref(), original_location.as_deref())
            {
                return Some((location.to_owned(), name.to_owned()));
            }
        }
        if next_node != 0 {
            pending.push(next_node);
        }
    }

    None
}

struct Record {
    name: String,
    key: [u8; 4],
    string_value: Option<String>,
}

fn read_record(cursor: &mut Cursor<'_>) -> Option<Record> {
    let name_len = usize::try_from(cursor.u32()?).ok()?;
    let name_bytes = cursor.take(name_len.checked_mul(2)?)?;
    let name = decode_utf16be(name_bytes);
    let key = cursor.array4()?;
    let data_type = cursor.array4()?;
    let string_value = read_value(cursor, data_type)?;
    Some(Record {
        name,
        key,
        string_value,
    })
}

fn read_value(cursor: &mut Cursor<'_>, data_type: [u8; 4]) -> Option<Option<String>> {
    match &data_type {
        b"bool" => cursor.skip(1).map(|()| None),
        b"long" | b"shor" | b"type" => cursor.skip(4).map(|()| None),
        b"comp" | b"dutc" => cursor.skip(8).map(|()| None),
        b"blob" => {
            let length = usize::try_from(cursor.u32()?).ok()?;
            cursor.skip(length).map(|()| None)
        }
        b"ustr" => {
            let length = usize::try_from(cursor.u32()?).ok()?;
            let bytes = cursor.take(length.checked_mul(2)?)?;
            Some(Some(decode_utf16be(bytes)))
        }
        _ => None,
    }
}

fn decode_utf16be(bytes: &[u8]) -> String {
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair));
    String::from_utf16_lossy(&units.collect::<Vec<_>>())
}

fn block(data: &[u8], offset: u32, size: u32) -> Option<&[u8]> {
    let start = usize::try_from(offset).ok()?.checked_add(4)?;
    let end = start.checked_add(usize::try_from(size).ok()?)?;
    data.get(start..end)
}

fn block_by_id<'a>(data: &'a [u8], offsets: &[u32], id: u32) -> Option<&'a [u8]> {
    let address = *offsets.get(usize::try_from(id).ok()?)?;
    let size = 1u32.checked_shl(address & 0x1f)?;
    block(data, address & !0x1f, size)
}

struct Cursor<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.position.checked_add(length)?;
        let bytes = self.data.get(self.position..end)?;
        self.position = end;
        Some(bytes)
    }

    fn array4(&mut self) -> Option<[u8; 4]> {
        self.take(4)?.try_into().ok()
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.array4()?))
    }

    fn u8(&mut self) -> Option<u8> {
        Some(*self.take(1)?.first()?)
    }

    fn skip(&mut self, length: usize) -> Option<()> {
        self.take(length).map(|_| ())
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
#[path = "macos_ds_store/tests.rs"]
mod tests;
