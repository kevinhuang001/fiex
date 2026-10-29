use serde::Serialize;
use crate::core::files::Listing;
use crate::plugins::resolvers::decorations::Decoration;

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
        let broker = crate::core::fs::trusted(&path, "core:directory-open")?;
        broker.require(crate::plugins::provider_registry::FsOperation::List, false)?;
        let send = |snapshot: crate::core::fs::directory_cache::DirectorySnapshot, complete| {
            let _ = on_snapshot.send(DirectoryViewUpdate {
                listing: snapshot.listing,
                decorations: snapshot.decorations,
                complete,
            });
        };
        if let Some(cached) =
            crate::core::fs::directory_cache::read_snapshot(&path, hidden, &ignores)?
        {
            send(cached, false);
        }
        let listing = broker.desktop_listing(hidden, &ignores).map_err(String::from)?;
        if let Some(fresh) =
            crate::core::fs::directory_cache::read_snapshot(&path, hidden, &ignores)?
        {
            send(fresh, false);
        }
        let entries: Vec<crate::plugins::resolvers::decorations::DecorationEntry> = listing
            .entries.iter()
            .filter_map(|entry| serde_json::to_value(entry).ok()
                .and_then(|value| serde_json::from_value(value).ok()))
            .collect();
        let decorations = crate::plugins::resolvers::decorations::query_with_plugins(
            &path, entries.clone(), true,
        );
        crate::core::fs::directory_cache::store_decorations(&path, &entries, &decorations);
        if let Some(fresh) =
            crate::core::fs::directory_cache::read_snapshot(&path, hidden, &ignores)?
        {
            send(fresh, true);
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| error.to_string())?
}
