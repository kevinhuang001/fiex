use super::*;
use crate::core::files::Entry;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};

#[derive(Clone, Debug)]
struct PendingReservation {
    target: PendingTarget,
    created_at: Instant,
}

fn pending_reservations() -> &'static Mutex<HashMap<String, PendingReservation>> {
    static RESERVATIONS: OnceLock<Mutex<HashMap<String, PendingReservation>>> = OnceLock::new();
    RESERVATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn reserve_child(parent_uri: &str, name: &str, kind: PendingKind) -> Result<String, String> {
    crate::core::files::validate_name(name)?;
    let parent = folder_id_or_root(parent_uri)?;
    let state = snapshot()?;
    if !parent_uri_matches(&state, parent_uri) {
        return Err("StaleRef: Applications parent moved".into());
    }
    require_parent(&state, parent.as_deref())?;
    if let Some(folder) = state
        .folders
        .values()
        .find(|folder| folder.parent == parent && folder.name.eq_ignore_ascii_case(name))
    {
        return folder_uri_in(&state, &folder.id)
            .ok_or_else(|| "invalid Applications folder ancestry".into());
    }
    if let Some(app) = state
        .apps
        .values()
        .find(|app| app.parent == parent && app.display_name.eq_ignore_ascii_case(name))
    {
        return app_uri_in(&state, &app.id)
            .ok_or_else(|| "invalid Applications entry ancestry".into());
    }
    ensure_unique_name(&state, parent.as_deref(), name, None)?;
    let mut reservations = pending_reservations()
        .lock()
        .map_err(|_| "Applications reservation registry unavailable")?;
    reservations.retain(|_, pending| pending.created_at.elapsed().as_secs() < 600);
    if let Some(existing) = reservations
        .values()
        .find(|pending| pending.target.parent_uri == parent_uri && pending.target.name == name)
    {
        if existing.target.kind == kind {
            return Ok(existing.target.uri.clone());
        }
        return Err("Conflict: application child already reserved with a different kind".into());
    }
    let id = random_id()?;
    let extension = std::path::Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !valid_extension(&extension) {
        return Err("invalid application file extension".into());
    }
    let target = PendingTarget {
        uri: match kind {
            PendingKind::Folder => child_uri(parent_uri, &format!("{FOLDER_SEGMENT}{id}")),
            PendingKind::Application => child_uri(parent_uri, &format!("{APP_SEGMENT}{id}")),
        },
        parent_uri: parent_uri.to_owned(),
        name: name.to_owned(),
        asset_id: (kind == PendingKind::Application)
            .then(random_id)
            .transpose()?,
        extension,
        kind,
    };
    reservations.insert(
        target.uri.clone(),
        PendingReservation {
            target: target.clone(),
            created_at: Instant::now(),
        },
    );
    Ok(target.uri)
}

pub fn reserved_child(parent_uri: &str, name: &str) -> Result<PendingTarget, String> {
    let reservations = pending_reservations()
        .lock()
        .map_err(|_| "Applications reservation registry unavailable")?;
    reservations
        .values()
        .find(|pending| {
            pending.target.parent_uri == parent_uri
                && pending.target.name == name
                && pending.created_at.elapsed().as_secs() < 600
        })
        .map(|pending| pending.target.clone())
        .ok_or_else(|| "NotFound: Applications child reservation".into())
}

pub fn pending_target(uri: &str) -> Option<PendingTarget> {
    pending_reservations()
        .lock()
        .ok()?
        .get(uri)
        .filter(|pending| pending.created_at.elapsed().as_secs() < 600)
        .map(|pending| pending.target.clone())
}

pub fn cancel_reservation(uri: &str) {
    if let Ok(mut reservations) = pending_reservations().lock() {
        reservations.remove(uri);
    }
}

pub fn commit_reserved_folder(uri: &str) -> Result<Entry, String> {
    let target = pending_target(uri).ok_or("NotFound: Applications child reservation")?;
    if target.kind != PendingKind::Folder {
        return Err("InvalidVfsUri: reserved target is not a folder".into());
    }
    let id = folder_id(uri).ok_or("invalid reserved folder URI")?;
    let parent = folder_id_or_root(&target.parent_uri)?;
    let state = update(|state| {
        require_parent(state, parent.as_deref())?;
        if !parent_uri_matches(state, &target.parent_uri) {
            return Err("StaleRef: Applications parent moved".into());
        }
        ensure_unique_name(state, parent.as_deref(), &target.name, None)?;
        if state.folders.contains_key(&id) {
            return Err("Conflict: application folder already exists".into());
        }
        let order = next_order(state, parent.as_deref());
        state.folders.insert(
            id.clone(),
            VirtualFolder {
                id: id.clone(),
                parent: parent.clone(),
                name: target.name.clone(),
                order,
            },
        );
        Ok(())
    })?;
    cancel_reservation(uri);
    Ok(folder_entry(
        state.folders.get(&id).expect("folder was inserted"),
        &state,
    ))
}

pub fn commit_reserved_application(uri: &str) -> Result<Entry, String> {
    let target = pending_target(uri).ok_or("NotFound: Applications child reservation")?;
    if target.kind != PendingKind::Application {
        return Err("InvalidVfsUri: reserved target is not an application".into());
    }
    let id = app_id(uri).ok_or("invalid reserved application URI")?;
    let asset_id = target
        .asset_id
        .clone()
        .ok_or("missing application asset ID")?;
    let asset = imported_asset_path(&asset_id, &target.extension)?;
    if !asset.exists() {
        return Err("application asset was not committed".into());
    }
    let parent = folder_id_or_root(&target.parent_uri)?;
    let state = update(|state| {
        require_parent(state, parent.as_deref())?;
        if !parent_uri_matches(state, &target.parent_uri) {
            return Err("StaleRef: Applications parent moved".into());
        }
        ensure_unique_name(state, parent.as_deref(), &target.name, None)?;
        if state.apps.contains_key(&id) {
            return Err("Conflict: application already exists".into());
        }
        let order = next_order(state, parent.as_deref());
        state.apps.insert(
            id.clone(),
            ApplicationNode {
                id: id.clone(),
                parent: parent.clone(),
                display_name: target.name.clone(),
                source: ApplicationSource::Imported {
                    asset_id: asset_id.clone(),
                    extension: target.extension.clone(),
                },
                order,
                unavailable: false,
            },
        );
        Ok(())
    })?;
    cancel_reservation(uri);
    Ok(app_entry(
        state.apps.get(&id).expect("application was inserted"),
        &state,
    ))
}

pub(crate) fn random_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|error| error.to_string())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let encoded = hex::encode(bytes);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &encoded[0..8],
        &encoded[8..12],
        &encoded[12..16],
        &encoded[16..20],
        &encoded[20..32]
    ))
}
