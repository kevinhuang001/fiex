use crate::core::files::Listing;
use crate::plugins::resolvers::decorations::Decoration;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectoryViewUpdate {
    pub listing: Listing,
    pub decorations: Vec<Decoration>,
    pub complete: bool,
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
        let started = std::time::Instant::now();
        let identity = crate::core::fs::directory_cache::key(&path);
        let cached = crate::core::fs::directory_cache::read_snapshot(&path, hidden, &ignores)?;
        // Warm listings paint an atomic metadata + Decoration snapshot.
        // A cold listing waits for its first complete presentation rather
        // than displaying generic icons which will immediately flash.
        if let Some(cached) = cached {
            let decorations = crate::core::fs::decoration_cache::resolve(&path, &cached.listing);
            let _ = on_snapshot.send(DirectoryViewUpdate {
                listing: cached.listing,
                decorations,
                complete: false,
            });
        }
        let broker = crate::core::fs::trusted(&path, "core:directory-open")?;
        let listing = broker
            .desktop_listing(hidden, &ignores)
            .map_err(String::from)?;
        let decorations = crate::core::fs::decoration_cache::resolve(&path, &listing);
        if identity != crate::core::fs::directory_cache::key(&path) {
            return Err("StaleRef: connection identity changed during listing".into());
        }
        let count = listing.entries.len();
        let _ = on_snapshot.send(DirectoryViewUpdate {
            listing,
            decorations,
            complete: true,
        });
        crate::infrastructure::logging::record_core(
            "debug",
            "fs.list",
            &format!(
                "Directory listed {} entries with presentation in {} ms",
                count,
                started.elapsed().as_millis(),
            ),
        );
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| error.to_string())?
}
