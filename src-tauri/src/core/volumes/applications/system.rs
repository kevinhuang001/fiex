//! Installed applications retain stable IDs when users move them into folders.
use super::{catalog, entry, state};
use crate::core::files::Entry;

fn catalog_entries() -> Result<Vec<fiex_applications::CatalogApplication>, String> {
    state::ensure_ready()?;
    #[cfg(feature = "desktop")]
    let on_change = crate::desktop::emit_applications_changed as fn();
    #[cfg(not(feature = "desktop"))]
    let on_change = || {};
    catalog().entries_cache_first_background(on_change)
}

/// All installed applications, including those currently mapped into folders.
pub fn system_entries() -> Result<Vec<Entry>, String> {
    catalog_entries().map(|entries| entries.into_iter().map(catalog_entry).collect())
}

pub fn system_entry(path: &str) -> Result<Entry, String> {
    let node = state::source(path)?;
    if !matches!(node.source, state::ApplicationSource::System { .. }) {
        return Err("not a system application".into());
    }

    match catalog_entries() {
        Ok(entries) => {
            if let Some(application) = entries.iter().find(|application| application.uri == path) {
                return Ok(catalog_entry(application.clone()));
            }
            // The catalog cache can still carry the pre-move URI until its
            // background refresh completes. Match the durable app identity,
            // then project the current tree URI instead of rejecting it.
            if entries
                .iter()
                .any(|application| application.stable_id() == node.id)
            {
                return state::entry(path);
            }
        }
        Err(error) if !node.unavailable => return Err(error),
        Err(_) => {}
    }
    if node.unavailable {
        return state::entry(path);
    }
    Err(crate::i18n::t("应用不存在").to_string())
}

fn catalog_entry(application: fiex_applications::CatalogApplication) -> Entry {
    let mut item = entry(
        application.display_name,
        application.uri,
        &application.extension,
        application.modified,
    );
    item.link_target = application.icon_source;
    item
}
