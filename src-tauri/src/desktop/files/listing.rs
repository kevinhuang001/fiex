use super::*;

#[tauri::command]
pub(crate) async fn list_directory(
    path: String,
    hidden: bool,
    ignores: Vec<String>,
    recent: tauri::State<'_, recent::Recent>,
) -> Result<files::Listing, String> {
    if let Some(resolver) = crate::core::volumes::virtual_resolvers::resolver_for(&path) {
        let recent_entries = recent::entries(&recent);
        let ctx = crate::core::volumes::virtual_resolvers::VirtualContext {
            recent: &recent_entries,
        };
        return Ok(files::Listing {
            read_only: resolver.readonly(),
            can_create: false,
            virtual_volume: true,
            path,
            entries: resolver.list(&ctx)?,
            warnings: vec![],
            default_view: resolver.default_view().map(str::to_owned),
        });
    }
    if remote::is_remote(&path) {
        return remote::list_listing(&path, hidden, &ignores)
            .await?
            .ok_or_else(|| "MountNotFound: connection removed".into());
    }
    if crate::plugins::vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::plugins::vfs::list(&path, hidden, &ignores)
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    if let Some(route) = crate::usbfs::route(&path) {
        let listing_path = path.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let mut entries = crate::usbfs::listing(&route)?;
            let filter = crate::core::ignores::IgnoreSet::new(&ignores);
            entries.retain(|entry| {
                (hidden || !entry.name.starts_with('.'))
                    && !filter.is_ignored(std::path::Path::new(&entry.name))
            });
            let read_only = matches!(
                route.kind,
                crate::usbfs::FsKind::Apfs
                    | crate::usbfs::FsKind::Hfs
                    | crate::usbfs::FsKind::Btrfs
            );
            Ok(files::Listing {
                read_only,
                can_create: !read_only,
                virtual_volume: true,
                path: listing_path,
                entries,
                warnings: vec![],
                default_view: None,
            })
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    tauri::async_runtime::spawn_blocking(move || files::list(&path, hidden, &ignores))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn file_info(path: String) -> Result<files::Entry, String> {
    tauri::async_runtime::spawn_blocking(move || file_info_blocking(&path))
        .await
        .map_err(|error| error.to_string())?
}

pub(super) fn file_info_blocking(path: &str) -> Result<files::Entry, String> {
    if remote::is_remote(path) {
        return remote::stat(path);
    }
    if crate::plugins::vfs::handles(path) {
        return crate::plugins::vfs::stat(path);
    }
    if let Some(route) = crate::usbfs::route(path) {
        return crate::usbfs::entry(&route);
    }
    files::entry(std::path::Path::new(path))
}

#[tauri::command]
pub(crate) async fn read_preview(path: String) -> Result<String, String> {
    if remote::is_remote(&path) {
        let path = remote::ensure_downloaded(&path, &remote::TransferOptions::default()).await?;
        return tauri::async_runtime::spawn_blocking(move || files::text_preview(&path))
            .await
            .map_err(|e| e.to_string())?;
    }
    if crate::plugins::vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            let bytes = crate::plugins::vfs::read_prefix(&path, 512 * 1024)?;
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(route) = crate::usbfs::route(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            let bytes = crate::usbfs::read_path(&route)?;
            Ok(String::from_utf8_lossy(&bytes[..bytes.len().min(512 * 1024)]).into_owned())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    tauri::async_runtime::spawn_blocking(move || files::text_preview(&path))
        .await
        .map_err(|e| e.to_string())?
}
