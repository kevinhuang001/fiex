//! Bounded host filesystem search primitives shared by native search adapters.
//!
//! The crate owns traversal, content limits, hidden-file behavior, and bundle
//! pruning. The application supplies the resource-ignore policy and resolves
//! matching paths into its own entry DTO before streaming them onward.

use fiex_provider_contracts::SearchMode;
use ignore::WalkBuilder;
use std::{path::Path, sync::Arc};

mod providers;

pub use providers::{
    SearchModule, SearchModuleServices, SearchProviderRegistration, MODULE_ID, PROVIDER_ID,
};

pub const CONTENT_FILE_LIMIT: u64 = 2 * 1024 * 1024;
pub const BINARY_PROBE: usize = 8192;
const CONTENT_BYTE_BUDGET: u64 = 64 * 1024 * 1024;
const CONTENT_FILE_BUDGET: usize = 5000;
const BUNDLE_EXTENSIONS: [&str; 5] = ["app", "bundle", "framework", "plugin", "xpc"];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchWalkStats {
    pub truncated: bool,
    pub skipped: usize,
    pub scanned: usize,
    pub incomplete: bool,
}

/// Walk one host directory and resolve each matching path through the caller.
///
/// Ignore semantics and entry creation remain owned by the caller, while this
/// crate owns traversal and bounded content scanning. A resolver error skips
/// that item (for example, if it disappeared during enumeration); an emission
/// error stops the walk and is returned to the caller.
#[allow(clippy::too_many_arguments)]
pub fn walk<T>(
    root: &Path,
    query: &str,
    mode: SearchMode,
    hidden: bool,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    ignored: Arc<dyn Fn(&Path) -> bool + Send + Sync>,
    mut resolve: impl FnMut(&Path) -> Result<T, String>,
    mut emit: impl FnMut(T) -> Result<(), String>,
) -> Result<SearchWalkStats, String> {
    let mut stats = SearchWalkStats::default();
    let lowered = query.to_lowercase();
    if !root.is_dir() || lowered.is_empty() {
        return Ok(stats);
    }

    let root = root.to_path_buf();
    let filter_root = root.clone();
    let walker = WalkBuilder::new(&root)
        .hidden(!hidden)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .same_file_system(true)
        .threads(1)
        .filter_entry(move |item| {
            if item.depth() == 0 {
                return true;
            }
            !inside_bundle(item.path())
                && !ignored(
                    item.path()
                        .strip_prefix(&filter_root)
                        .unwrap_or(item.path()),
                )
        })
        .build();

    let mut content = ContentBudget::default();
    let mut emitted = 0usize;
    for item in walker {
        if cancelled() {
            break;
        }
        let Ok(item) = item else {
            stats.skipped += 1;
            continue;
        };
        if item.depth() == 0 {
            continue;
        }
        stats.scanned += 1;
        let path = item.path();
        let matches = match mode {
            SearchMode::Name => item
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .contains(&lowered),
            SearchMode::Content => {
                if path.is_dir() {
                    continue;
                }
                match content.match_file(path, &lowered) {
                    ContentMatch::Hit => true,
                    ContentMatch::Miss => false,
                    ContentMatch::Skipped => {
                        stats.skipped += 1;
                        false
                    }
                    ContentMatch::Exhausted => {
                        stats.incomplete = true;
                        break;
                    }
                }
            }
        };
        if !matches {
            continue;
        }
        let value = match resolve(path) {
            Ok(value) => value,
            Err(_) => {
                stats.skipped += 1;
                continue;
            }
        };
        if emitted >= limit {
            stats.truncated = true;
            break;
        }
        emit(value)?;
        emitted += 1;
    }
    Ok(stats)
}

#[derive(Default)]
struct ContentBudget {
    files: usize,
    bytes: u64,
}

enum ContentMatch {
    Hit,
    Miss,
    Skipped,
    Exhausted,
}

impl ContentBudget {
    fn match_file(&mut self, path: &Path, lowered: &str) -> ContentMatch {
        if self.files >= CONTENT_FILE_BUDGET || self.bytes >= CONTENT_BYTE_BUDGET {
            return ContentMatch::Exhausted;
        }
        let Ok(metadata) = std::fs::metadata(path) else {
            return ContentMatch::Skipped;
        };
        if metadata.len() > CONTENT_FILE_LIMIT {
            return ContentMatch::Skipped;
        }
        let Ok(bytes) = std::fs::read(path) else {
            return ContentMatch::Skipped;
        };
        self.files += 1;
        self.bytes += bytes.len() as u64;
        let probe = &bytes[..bytes.len().min(BINARY_PROBE)];
        if probe.contains(&0) {
            return ContentMatch::Skipped;
        }
        if String::from_utf8_lossy(&bytes)
            .to_lowercase()
            .contains(lowered)
        {
            ContentMatch::Hit
        } else {
            ContentMatch::Miss
        }
    }
}

fn is_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            BUNDLE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

fn inside_bundle(path: &Path) -> bool {
    path.ancestors().skip(1).any(is_bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Mutex};

    fn collect(root: &Path, query: &str, mode: SearchMode, hidden: bool) -> Vec<PathBuf> {
        let found = Mutex::new(Vec::new());
        let stats = walk(
            root,
            query,
            mode,
            hidden,
            usize::MAX,
            &|| false,
            Arc::new(|_: &Path| false),
            |path| Ok(path.to_path_buf()),
            |path| {
                found.lock().unwrap().push(path);
                Ok(())
            },
        )
        .unwrap();
        assert!(!stats.truncated);
        found.into_inner().unwrap()
    }

    #[test]
    fn filename_walk_skips_bundle_contents_and_respects_hidden_setting() {
        let directory = tempfile::tempdir().unwrap();
        let bundle = directory.path().join("Editor.app");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents/Needle.plist"), "x").unwrap();
        std::fs::write(directory.path().join("Needle.txt"), "x").unwrap();
        std::fs::write(directory.path().join(".Needle-hidden"), "x").unwrap();

        let visible = collect(directory.path(), "needle", SearchMode::Name, false);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].file_name().unwrap(), "Needle.txt");

        let hidden = collect(directory.path(), "needle", SearchMode::Name, true);
        assert_eq!(hidden.len(), 2);
    }

    #[test]
    fn content_walk_matches_text_and_skips_binary_files() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("match.txt"), "hello 世界").unwrap();
        std::fs::write(directory.path().join("binary.dat"), b"hello\0world").unwrap();

        let found = collect(directory.path(), "世界", SearchMode::Content, false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file_name().unwrap(), "match.txt");
    }

    #[test]
    fn walker_reports_truncation_only_after_an_extra_match() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..3 {
            std::fs::write(directory.path().join(format!("Needle-{index}.txt")), "x").unwrap();
        }
        let found = Mutex::new(Vec::new());
        let stats = walk(
            directory.path(),
            "needle",
            SearchMode::Name,
            false,
            2,
            &|| false,
            Arc::new(|_: &Path| false),
            |path| Ok(path.to_path_buf()),
            |path| {
                found.lock().unwrap().push(path);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(found.into_inner().unwrap().len(), 2);
        assert!(stats.truncated);
    }
}
