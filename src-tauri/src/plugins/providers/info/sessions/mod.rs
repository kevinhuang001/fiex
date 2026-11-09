//! Core-owned Info sessions, field caching and task cancellation.
use super::{InfoCompute, InfoFieldKey, InfoFieldResult, InfoSectionDescriptor};
use crate::plugins::{events::Resolver, resource::resource_context::ResourceSnapshot};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod cache;
mod compute;
mod initialize;
pub(super) use cache::{ComputePermit, cache_core_results, read_core_cache};
pub(crate) use compute::{ComputeTrigger, compute_fields, compute_session};
pub(crate) use initialize::initialize_session;

const SESSION_TTL: Duration = Duration::from_secs(900);
const SESSION_LIMIT: usize = 128;
pub(super) const FIELD_LIMIT: usize = 64;

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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoOpenResult {
    pub session_id: String,
    pub sections: Vec<InfoSectionView>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoFieldUpdate {
    pub provider_id: String,
    pub section_id: String,
    pub field_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoFieldDelta {
    pub session_id: String,
    pub revision: u64,
    pub updates: Vec<InfoFieldUpdate>,
}

#[derive(Clone)]
pub(super) struct ProviderState {
    pub resolver: Resolver,
    pub sections: Vec<InfoSectionDescriptor>,
}

pub(super) struct Session {
    pub resource: ResourceSnapshot,
    pub version: String,
    pub opened: Instant,
    pub sections: Vec<InfoSectionView>,
    pub providers: HashMap<String, ProviderState>,
    pub tasks: HashMap<String, Arc<AtomicBool>>,
    pub revision: u64,
}

pub(super) fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn version(resource: &ResourceSnapshot) -> String {
    let local = crate::plugins::cache::key("info.version", &resource.local);
    serde_json::to_string(&(
        &resource.reference,
        resource.metadata(),
        resource.operations.map(|value| value.into_allowed_ops()),
        local,
    ))
    .unwrap_or_default()
}

pub(super) fn new_id(prefix: &str) -> String {
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

pub(super) fn insert_session(id: String, session: Session) -> Result<(), String> {
    let mut store = sessions()
        .lock()
        .map_err(|_| "Info session store unavailable")?;
    expire_sessions(&mut store);
    store.insert(id, session);
    Ok(())
}

pub(super) fn update_fields(
    session_id: &str,
    provider_id: &str,
    requested: &[InfoFieldKey],
    results: &[InfoFieldResult],
    error: Option<&str>,
) -> Result<InfoFieldDelta, String> {
    let mut store = sessions()
        .lock()
        .map_err(|_| "Info session store unavailable")?;
    let session = store.get_mut(session_id).ok_or("Info session expired")?;
    let mut updates = Vec::new();
    for section in session
        .sections
        .iter_mut()
        .filter(|section| section.provider_id == provider_id)
    {
        for field in &mut section.fields {
            let Some(request) = requested.iter().find(|request| {
                request.section_id == field.section_id && request.field_id == field.id
            }) else {
                continue;
            };
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
                field.value = None;
                field.error = Some(crate::infrastructure::redact::clean(error, 1500));
            }
            updates.push(InfoFieldUpdate {
                provider_id: provider_id.to_owned(),
                section_id: field.section_id.clone(),
                field_id: field.id.clone(),
                status: field.status.clone(),
                value: field.value.clone(),
                error: field.error.clone(),
            });
        }
    }
    if !updates.is_empty() {
        session.revision = session.revision.wrapping_add(1);
    }
    Ok(InfoFieldDelta {
        session_id: session_id.to_owned(),
        revision: session.revision,
        updates,
    })
}

pub(crate) fn mark_fields_error(
    session_id: &str,
    provider_id: &str,
    fields: &[InfoFieldKey],
    error: &str,
) -> Result<InfoFieldDelta, String> {
    update_fields(session_id, provider_id, fields, &[], Some(error))
}

pub(crate) fn cancel_session_task(session_id: &str, task_id: &str) {
    if let Ok(store) = sessions().lock()
        && let Some(cancelled) = store
            .get(session_id)
            .and_then(|session| session.tasks.get(task_id))
    {
        cancelled.store(true, Ordering::Release);
    }
}

pub(crate) fn close_session(session_id: &str) {
    if let Ok(mut store) = sessions().lock()
        && let Some(session) = store.remove(session_id)
    {
        for cancelled in session.tasks.values() {
            cancelled.store(true, Ordering::Release);
        }
    }
}

pub(crate) fn forget_plugin(plugin_id: &str) {
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
