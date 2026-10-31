//! Bounded VFS/USBFS content-search fallback when a native content-search
//! implementation is unavailable. Every read uses ResourceBroker's scoped
//! storage owner rather than materializing a remote file in a host path.
use super::*;
use std::collections::HashSet;

const MAX_DIRECTORIES: usize = 1000;
const MAX_REMOTE_FILES: usize = 1024;
const MAX_REMOTE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn run(
    root: &str,
    request: &SearchRequest,
    cancelled: &dyn Fn() -> bool,
    sink: &mut BatchSink<'_>,
) -> Result<(), String> {
    let mut pending = vec![root.to_string()];
    let mut seen = HashSet::new();
    let mut files = 0usize;
    let mut bytes = 0u64;
    let needle = request.query.to_lowercase();
    let ignores = IgnoreSet::new(&request.ignores);

    while let Some(directory) = pending.pop() {
        if cancelled() {
            return Ok(());
        }
        if !seen.insert(directory.clone()) {
            continue;
        }
        if seen.len() > MAX_DIRECTORIES {
            sink.incomplete = true;
            sink.warning = Some("content search reached the directory budget".into());
            return Ok(());
        }
        let listing = crate::core::fs::trusted(&directory, "core:search")?
            .desktop_listing(request.hidden, &request.ignores)?;
        for entry in listing.entries {
            if cancelled() {
                return Ok(());
            }
            sink.scanned += 1;
            let relative = entry
                .path
                .strip_prefix(root)
                .unwrap_or(&entry.name)
                .trim_start_matches(['/', '\\']);
            if ignores.is_ignored(Path::new(relative)) {
                continue;
            }
            if entry.is_dir && !entry.is_symlink {
                pending.push(entry.path);
                continue;
            }
            if entry.is_dir || entry.is_symlink || entry.size > CONTENT_FILE_LIMIT {
                sink.skipped += 1;
                continue;
            }
            if files >= MAX_REMOTE_FILES || bytes >= MAX_REMOTE_BYTES {
                sink.incomplete = true;
                sink.warning = Some("content search reached the network/read budget".into());
                return Ok(());
            }
            files += 1;
            let limit = (MAX_REMOTE_BYTES - bytes).min(CONTENT_FILE_LIMIT) as usize;
            let file = match crate::core::fs::trusted(&entry.path, "core:search") {
                Ok(file) => file,
                Err(_) => {
                    sink.skipped += 1;
                    continue;
                }
            };
            let content = match file.preview_prefix(limit) {
                Ok(content) => content,
                Err(_) => {
                    sink.skipped += 1;
                    continue;
                }
            };
            bytes += content.len() as u64;
            if text_contains(&content, &needle) {
                sink.push(entry);
                if sink.total >= sink.limit {
                    sink.truncated = true;
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn text_contains(bytes: &[u8], lowered: &str) -> bool {
    !bytes[..bytes.len().min(BINARY_PROBE)].contains(&0)
        && String::from_utf8_lossy(bytes)
            .to_lowercase()
            .contains(lowered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_search_probe_rejects_binary_and_matches_unicode() {
        assert!(text_contains("hello 世界".as_bytes(), "世界"));
        assert!(!text_contains(b"abc\0hello", "hello"));
        assert!(!text_contains(b"hello world", "missing"));
    }
}
