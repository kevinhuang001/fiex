//! Cache eligibility and current account identity, without secret hydration.
pub(crate) fn enabled(path: &str) -> bool {
    crate::usbfs::route(path).is_none()
        && (crate::plugins::mounts::handles(path)
            || crate::core::volumes::virtual_resolvers::view_for(path).is_some())
}

pub(crate) fn key(path: &str) -> String {
    // A URI's mount ID alone is not sufficient when a saved connection changes
    // accounts, settings or provider. Include its current authorized identity.
    if crate::plugins::mounts::handles(path) {
        use sha2::{Digest, Sha256};
        if let Ok(uri) = crate::plugins::mounts::parse(path) {
            // Cache identity is built from the public connection record: do
            // not hydrate secrets or trigger OS keyring prompts on cache reads.
            let identity = crate::remote::load_stored_public()
                .into_iter()
                .find(|conn| conn.mount_id == uri.mount_id)
                .and_then(|conn| {
                    serde_json::to_vec(&(
                        conn.id,
                        conn.plugin_id,
                        conn.kind,
                        conn.settings,
                        conn.mount_id,
                        conn.cache_revision,
                    ))
                    .ok()
                })
                .or_else(|| {
                    crate::plugins::mounts::view(&uri.mount_id)
                        .ok()
                        .and_then(|view| serde_json::to_vec(&view.descriptor).ok())
                });
            if let Some(identity) = identity {
                return format!("mount:{}:{}", hex::encode(Sha256::digest(&identity)), path);
            }
        }
    }
    if let Some(virtual_path) = crate::core::volumes::virtual_storage::virtual_path_key(path) {
        return virtual_path;
    }
    path.into()
}
pub(super) fn validate_mount(path: &str) -> Result<(), String> {
    // Reauthorize every cache read, but never hydrate remote credentials:
    // providers see exactly the same permissions while cached navigation
    // avoids a keyring prompt on every paint and filename search.
    if crate::core::volumes::trash_vfs::handles(path) {
        crate::core::volumes::trash_vfs::check_access()?;
    }
    if crate::plugins::mounts::handles(path) {
        let uri = crate::plugins::mounts::parse(path)?;
        if let Some(connection) = crate::remote::load_stored_public()
            .into_iter()
            .find(|conn| conn.mount_id == uri.mount_id)
        {
            crate::remote::ensure_provider_access(&connection)?;
        } else {
            crate::plugins::mounts::view(&uri.mount_id)?;
        }
    }
    Ok(())
}
