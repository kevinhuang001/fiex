//! Session-bound Open With choices. Only opaque menu tokens cross the UI
//! boundary; platform application identities remain in Core memory.

use fiex_platform::OpenWithCandidate;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const SESSION_TTL: Duration = Duration::from_secs(90);

struct CandidateBinding {
    application: OpenWithCandidate,
    source_path: String,
    extension: String,
    mime_type: Option<String>,
    expires: Instant,
}

fn bindings() -> &'static Mutex<HashMap<(String, String), CandidateBinding>> {
    static BINDINGS: OnceLock<Mutex<HashMap<(String, String), CandidateBinding>>> = OnceLock::new();
    BINDINGS.get_or_init(Default::default)
}

pub(super) fn menu_items(
    context: &super::builtin::Context,
    menu_session_id: &str,
) -> Result<Vec<fiex_provider_contracts::MenuItem>, String> {
    let Some(path) = context.paths.first() else {
        return Ok(Vec::new());
    };
    let Some(entry) = context.entries.first() else {
        return Ok(Vec::new());
    };
    if entry.is_dir || entry.is_symlink || !context.capabilities().read {
        return Ok(Vec::new());
    }
    let extension = entry.extension.trim_start_matches('.').to_ascii_lowercase();
    let probe = probe_path(&extension);
    let candidates = fiex_platform::open_with_candidates(&probe, &extension, Some(&entry.mime))?;
    let mut registry = bindings()
        .lock()
        .map_err(|_| "Open With session registry unavailable")?;
    registry.retain(|_, binding| binding.expires > Instant::now());
    let mut items = Vec::with_capacity(candidates.len() + 2);
    for candidate in candidates {
        let token = crate::plugins::mounts::new_id()?;
        let item_id = format!("openWith-{token}");
        registry.insert(
            (menu_session_id.to_owned(), item_id.clone()),
            CandidateBinding {
                application: candidate.clone(),
                source_path: path.clone(),
                extension: extension.clone(),
                mime_type: Some(entry.mime.clone()),
                expires: Instant::now() + SESSION_TTL,
            },
        );
        let mut item = fiex_provider_contracts::MenuItem::action(item_id, candidate.name);
        item.icon = Some("application".into());
        items.push(item);
    }
    if !items.is_empty() {
        items.push(fiex_provider_contracts::MenuItem {
            id: "openWith-separator".into(),
            kind: fiex_provider_contracts::MenuItemKind::Separator,
            title: " ".into(),
            parent_id: None,
            icon: None,
            shortcut: None,
            disabled: false,
            danger: false,
            placement: None,
            activity: None,
        });
    }
    let mut choose = fiex_provider_contracts::MenuItem::action(
        "openWith-choose",
        crate::i18n::t("选择其他应用…"),
    );
    choose.icon = Some("application-search".into());
    items.push(choose);
    if registry.len() > 8192 {
        registry.clear();
    }
    Ok(items)
}

pub(super) fn open_selected(
    context: &super::builtin::Context,
    menu_session_id: &str,
    item_id: &str,
) -> Result<(), String> {
    let binding = bindings()
        .lock()
        .map_err(|_| "Open With session registry unavailable")?
        .remove(&(menu_session_id.to_owned(), item_id.to_owned()))
        .filter(|binding| binding.expires > Instant::now())
        .ok_or("Open With selection expired")?;
    let path = context.paths.first().ok_or("Open With requires one file")?;
    if &binding.source_path != path {
        return Err("Open With selection belongs to a different file".into());
    }
    let user_action = crate::core::fs::mapped_access::UserActionContext::from_core_event();
    let exported =
        crate::core::fs::export_for_external_drag(std::slice::from_ref(path), user_action)?;
    let local = PathBuf::from(exported.first().ok_or("file export returned no path")?);
    let metadata = std::fs::symlink_metadata(&local).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Open With requires a regular file".into());
    }
    fiex_platform::open_with(
        &local,
        &binding.extension,
        binding.mime_type.as_deref(),
        &binding.application.id,
    )
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "desktop")]
pub(crate) struct PickerResponse {
    pub session_id: String,
    pub items: Vec<PickerItem>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "desktop")]
pub(crate) struct PickerItem {
    pub id: String,
    pub name: String,
    pub icon_source: Option<String>,
}

#[cfg(feature = "desktop")]
pub(crate) fn picker_candidates(path: String) -> Result<PickerResponse, String> {
    let user_action = crate::core::fs::mapped_access::UserActionContext::from_desktop_command();
    let resource =
        crate::core::fs::trusted_with_user_action(&path, "core:open-with-picker", user_action)?;
    resource.require(crate::plugins::provider_registry::FsOperation::Read, false)?;
    let entry = resource.desktop_entry().map_err(String::from)?;
    if entry.is_dir || entry.is_symlink {
        return Err("Open With requires a regular file".into());
    }
    let extension = entry.extension.trim_start_matches('.').to_ascii_lowercase();
    let candidates = fiex_platform::open_with_applications(&probe_path(&extension))?;
    let session_id = crate::plugins::mounts::new_id()?;
    let mut registry = bindings()
        .lock()
        .map_err(|_| "Open With session registry unavailable")?;
    registry.retain(|_, binding| binding.expires > Instant::now());
    let mut items = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let id = format!("openWith-{}", crate::plugins::mounts::new_id()?);
        registry.insert(
            (session_id.clone(), id.clone()),
            CandidateBinding {
                application: candidate.clone(),
                source_path: path.clone(),
                extension: extension.clone(),
                mime_type: Some(entry.mime.clone()),
                expires: Instant::now() + SESSION_TTL,
            },
        );
        items.push(PickerItem {
            id,
            name: candidate.name,
            icon_source: candidate.icon_source,
        });
    }
    if registry.len() > 8192 {
        registry.clear();
    }
    Ok(PickerResponse { session_id, items })
}

#[cfg(feature = "desktop")]
pub(crate) fn open_picker_choice(
    path: String,
    session_id: String,
    item_id: String,
) -> Result<(), String> {
    let binding = bindings()
        .lock()
        .map_err(|_| "Open With session registry unavailable")?
        .remove(&(session_id, item_id))
        .filter(|binding| binding.expires > Instant::now())
        .ok_or("Open With selection expired")?;
    if binding.source_path != path {
        return Err("Open With selection belongs to a different file".into());
    }
    let user_action = crate::core::fs::mapped_access::UserActionContext::from_desktop_command();
    let exported = crate::core::fs::export_for_external_drag(&[path], user_action)?;
    let local = PathBuf::from(exported.first().ok_or("file export returned no path")?);
    let metadata = std::fs::symlink_metadata(&local).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Open With requires a regular file".into());
    }
    fiex_platform::open_with_application(&local, &binding.application.id)
}

fn probe_path(extension: &str) -> PathBuf {
    let name = if extension.is_empty() {
        "fiex-open-with-probe".to_owned()
    } else {
        format!("fiex-open-with-probe.{extension}")
    };
    std::env::temp_dir().join(name)
}

pub(super) fn is_candidate_item(menu_session_id: &str, item_id: &str) -> bool {
    bindings()
        .lock()
        .ok()
        .and_then(|registry| {
            registry
                .get(&(menu_session_id.to_owned(), item_id.to_owned()))
                .map(|binding| binding.expires > Instant::now())
        })
        .unwrap_or(false)
}
