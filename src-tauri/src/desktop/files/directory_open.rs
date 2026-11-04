use crate::core::files::Listing;
use crate::plugins::providers::decorations::Decoration;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectoryViewUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listing: Option<Listing>,
    pub decorations: Vec<Decoration>,
    pub complete: bool,
    pub presentation_pending: bool,
}

/// One logical operation paints a previously visited directory and revalidates
/// metadata/decoration without exposing a cache API to the desktop.
#[tauri::command]
pub(crate) async fn directory_open(
    path: String,
    hidden: bool,
    ignores: Vec<String>,
    on_snapshot: tauri::ipc::Channel<DirectoryViewUpdate>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if crate::core::volumes::trash_vfs::handles(&path) {
            crate::core::volumes::trash_vfs::check_access()?;
        }
        let identity = crate::core::fs::directory_cache::key(&path);
        let cached = crate::core::fs::directory_cache::read_snapshot(&path, hidden, &ignores)?;
        let cache_hit = cached.is_some();
        // Paint a cached listing immediately. For stale snapshots, reuse only
        // decorations whose entry fingerprint still matches; fresh providers
        // are resolved after the first listing has reached the UI.
        if let Some(cached) = cached {
            let (decorations, exact) =
                crate::core::fs::decoration_cache::cached(&path, &cached.listing);
            let _ = on_snapshot.send(DirectoryViewUpdate {
                listing: Some(cached.listing),
                decorations,
                complete: false,
                presentation_pending: !exact,
            });
        }
        let listing_started = std::time::Instant::now();
        let broker = crate::core::fs::trusted(&path, "core:directory-open")?;
        let listing = broker
            .desktop_listing(hidden, &ignores)
            .map_err(String::from)?;
        let listing_elapsed = listing_started.elapsed();
        if identity != crate::core::fs::directory_cache::key(&path) {
            return Err("StaleRef: connection identity changed during listing".into());
        }
        let (decorations, exact) =
            crate::core::fs::decoration_cache::cached(&path, &listing);
        let _ = on_snapshot.send(DirectoryViewUpdate {
            listing: Some(listing.clone()),
            decorations: decorations.clone(),
            complete: exact,
            presentation_pending: !exact,
        });
        let presentation_started = std::time::Instant::now();
        if !exact {
            let decoration_patch =
                crate::core::fs::decoration_cache::resolve_missing(&path, &listing);
            if identity != crate::core::fs::directory_cache::key(&path) {
                return Err("StaleRef: connection identity changed during listing".into());
            }
            let _ = on_snapshot.send(DirectoryViewUpdate {
                listing: None,
                decorations: decoration_patch,
                complete: true,
                presentation_pending: false,
            });
        }
        let count = listing.entries.len();
        crate::infrastructure::logging::record_core(
            "debug",
            "host.fs.list",
            &format!(
                "listed {} entries in {} ms; presentation took {} ms (cache_hit={}, decoration_cache_hit={}, hidden={})",
                count,
                listing_elapsed.as_millis(),
                presentation_started.elapsed().as_millis(),
                cache_hit,
                exact,
                hidden,
            ),
        );
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| error.to_string())?
}
