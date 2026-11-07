//! Core-owned Info sessions, descriptor validation, field caching and task cancellation.
use super::{
    native, proxy::ScopedInfoProxy, InfoCompute, InfoFieldKey, InfoFieldResult,
    InfoSectionDescriptor,
};
use crate::plugins::{events::Resolver, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::InfoProvider;
use serde::Serialize;

mod cache;
mod compute;
mod describe;
pub(super) use cache::{cache_core_results, read_core_cache, ComputePermit};
pub(super) use compute::compute_fields;
pub(crate) use compute::compute_session;
pub(crate) use describe::describe_session;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SESSION_TTL: Duration = Duration::from_secs(900);
const SESSION_LIMIT: usize = 128;
const FIELD_LIMIT: usize = 64;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoFieldView {
    pub id: String,
    pub section_id: String,
    pub label: String,
    pub compute: InfoCompute,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoSectionView {
    pub id: String,
    pub title: String,
    pub provider_id: String,
    pub provider: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    pub fields: Vec<InfoFieldView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone)]
struct ProviderState {
    resolver: Resolver,
    sections: Vec<InfoSectionDescriptor>,
}

struct Session {
    resource: ResourceSnapshot,
    version: String,
    opened: Instant,
    sections: Vec<InfoSectionView>,
    providers: HashMap<String, ProviderState>,
    tasks: HashMap<String, Arc<AtomicBool>>,
}

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn version(resource: &ResourceSnapshot) -> String {
    let local = crate::plugins::cache::key("info.version", &resource.local);
    serde_json::to_string(&(
        &resource.reference,
        resource.metadata(),
        resource.operations.map(|value| value.into_allowed_ops()),
        local,
    ))
    .unwrap_or_default()
}

fn new_id(prefix: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{prefix}-{}-{time:x}-{:x}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn expire_sessions(sessions: &mut HashMap<String, Session>) {
    sessions.retain(|_, session| {
        if session.opened.elapsed() < SESSION_TTL {
            true
        } else {
            for cancelled in session.tasks.values() {
                cancelled.store(true, Ordering::Release);
            }
            false
        }
    });
    while sessions.len() >= SESSION_LIMIT {
        let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.opened)
            .map(|(id, _)| id.clone())
        else {
            break;
        };
        if let Some(session) = sessions.remove(&oldest) {
            for cancelled in session.tasks.values() {
                cancelled.store(true, Ordering::Release);
            }
        }
    }
}

pub fn open_session(path: String) -> Result<String, String> {
    let resource =
        crate::plugins::resource::resource_context::resolve_from_directory_snapshot(&path)?;
    let _ = crate::core::fs::trusted_snapshot(&resource, "core:info")?;
    let id = new_id("info");
    let session = Session {
        version: version(&resource),
        resource,
        opened: Instant::now(),
        sections: Vec::new(),
        providers: HashMap::new(),
        tasks: HashMap::new(),
    };
    let mut store = sessions()
        .lock()
        .map_err(|_| "Info session store unavailable")?;
    expire_sessions(&mut store);
    store.insert(id.clone(), session);
    Ok(id)
}

fn read_sections(session_id: &str) -> Result<Vec<InfoSectionView>, String> {
    let store = sessions()
        .lock()
        .map_err(|_| "Info session store unavailable")?;
    store
        .get(session_id)
        .map(|session| session.sections.clone())
        .ok_or_else(|| "Info session expired".into())
}

fn mark_fields_error(
    session_id: &str,
    provider_id: &str,
    fields: &[InfoFieldKey],
    error: &str,
) -> Result<(), String> {
    update_fields(session_id, provider_id, fields, &[], Some(error))
}

fn update_fields(
    session_id: &str,
    provider_id: &str,
    requested: &[InfoFieldKey],
    results: &[InfoFieldResult],
    error: Option<&str>,
) -> Result<(), String> {
    let mut store = sessions()
        .lock()
        .map_err(|_| "Info session store unavailable")?;
    let session = store.get_mut(session_id).ok_or("Info session expired")?;
    for section in session
        .sections
        .iter_mut()
        .filter(|section| section.provider_id == provider_id)
    {
        for field in &mut section.fields {
            if let Some(request) = requested.iter().find(|request| {
                request.section_id == field.section_id && request.field_id == field.id
            }) {
                if let Some(result) = results.iter().find(|result| {
                    result.section_id == request.section_id && result.field_id == request.field_id
                }) {
                    field.status = if result.unavailable {
                        "unavailable"
                    } else {
                        "ready"
                    }
                    .into();
                    field.value = result.value.clone();
                    field.error = None;
                } else if let Some(error) = error {
                    field.status = "error".into();
                    field.error = Some(crate::infrastructure::redact::clean(error, 1500));
                }
            }
        }
    }
    Ok(())
}

pub fn cancel_session_task(session_id: &str, task_id: &str) {
    if let Ok(store) = sessions().lock() {
        if let Some(cancelled) = store
            .get(session_id)
            .and_then(|session| session.tasks.get(task_id))
        {
            cancelled.store(true, Ordering::Release);
        }
    }
}

pub fn close_session(session_id: &str) {
    if let Ok(mut store) = sessions().lock() {
        if let Some(session) = store.remove(session_id) {
            for cancelled in session.tasks.values() {
                cancelled.store(true, Ordering::Release);
            }
        }
    }
}

pub fn forget_plugin(plugin_id: &str) {
    if let Ok(mut store) = sessions().lock() {
        for session in store.values_mut() {
            session
                .sections
                .retain(|section| section.plugin_id.as_deref() != Some(plugin_id));
            session.providers.retain(|_, provider| {
                provider
                    .resolver
                    .plugin
                    .as_deref()
                    .is_none_or(|plugin| plugin.id != plugin_id)
            });
        }
    }
}
