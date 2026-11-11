use super::super::Selection;
use fiex_resource::FileCapabilities;

pub(super) fn can_open(selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
    !selection.paths.is_empty()
        && selection.paths.iter().enumerate().all(|(index, path)| {
            let Some(entry) = selection.entries.get(index) else {
                return false;
            };
            crate::core::volumes::native_vfs::open_policy(path, entry)
                .unwrap_or_else(|| entry.is_dir || capabilities.read || entry.link_target.is_some())
        })
}
