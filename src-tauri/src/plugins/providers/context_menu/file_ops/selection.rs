use crate::core::files::Entry;
use fiex_resource::FileCapabilities;

/// The selection a menu is resolved for.
pub(in crate::plugins::providers::context_menu) struct Selection<'a> {
    pub(in crate::plugins::providers::context_menu) paths: &'a [String],
    pub(in crate::plugins::providers::context_menu) entries: &'a [Entry],
    pub(in crate::plugins::providers::context_menu) entry_capabilities: &'a [FileCapabilities],
    pub(in crate::plugins::providers::context_menu) directory_capabilities: FileCapabilities,
    pub(in crate::plugins::providers::context_menu) all_local: bool,
    pub(in crate::plugins::providers::context_menu) preflight:
        super::super::builtin::MenuPreflightAvailability,
}

impl Selection<'_> {
    pub(in crate::plugins::providers::context_menu) fn many(&self) -> bool {
        self.paths.len() > 1
    }
}
