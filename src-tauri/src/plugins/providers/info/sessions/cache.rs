use super::*;
use serde_json::json;
use std::sync::Condvar;

fn field_key(base: &str, provider_id: &str, field: &InfoFieldKey) -> Option<String> {
    if field.section_id == "directoryStats" {
        return None;
    }
    Some(format!(
        "{base}:{provider_id}:{}:{}",
        field.section_id, field.field_id
    ))
}

fn cache_base(resource: &ResourceSnapshot) -> Option<String> {
    let access = crate::core::fs::provider_registry::resource_access(&resource.display).ok()?;
    if access != crate::core::fs::provider_registry::ResourceAccess::HostPath {
        return None;
    }
    crate::plugins::cache::key("info.field", &resource.local)
}

pub fn read_core_cache(
    resource: &ResourceSnapshot,
    provider_id: &str,
    fields: &[InfoFieldKey],
    allow_plugin_cache: bool,
) -> Vec<InfoFieldResult> {
    if provider_id.starts_with("plugin:") && !allow_plugin_cache {
        return Vec::new();
    }
    let Some(base) = cache_base(resource) else {
        return Vec::new();
    };
    fields
        .iter()
        .filter_map(|field| {
            let key = field_key(&base, provider_id, field)?;
            let value = crate::plugins::cache::get(&key)?;
            let unavailable = value
                .get("unavailable")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let text = value
                .get("value")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            Some(InfoFieldResult {
                section_id: field.section_id.clone(),
                field_id: field.field_id.clone(),
                value: text,
                unavailable,
            })
        })
        .collect()
}

pub fn cache_core_results(
    resource: &ResourceSnapshot,
    provider_id: &str,
    requested: &[InfoFieldKey],
    results: &[InfoFieldResult],
    allow_plugin_cache: bool,
) {
    if provider_id.starts_with("plugin:") && !allow_plugin_cache {
        return;
    }
    let Some(base) = cache_base(resource) else {
        return;
    };
    for field in requested {
        if let (Some(key), Some(result)) = (
            field_key(&base, provider_id, field),
            results.iter().find(|value| {
                value.section_id == field.section_id && value.field_id == field.field_id
            }),
        ) {
            crate::plugins::cache::put(
                key,
                json!({"value": &result.value, "unavailable": result.unavailable}),
            );
        }
    }
}

pub struct ComputePermit;
impl ComputePermit {
    pub(super) fn acquire(cancelled: &AtomicBool) -> Result<Self, String> {
        let (active, ready) = limiter();
        let mut count = active
            .lock()
            .map_err(|_| "Info compute limiter unavailable")?;
        while *count >= 4 {
            if cancelled.load(Ordering::Acquire) {
                return Err("Info calculation cancelled".into());
            }
            let (next, _) = ready
                .wait_timeout(count, Duration::from_millis(50))
                .map_err(|_| "Info compute limiter unavailable")?;
            count = next;
        }
        *count += 1;
        Ok(Self)
    }
}
impl Drop for ComputePermit {
    fn drop(&mut self) {
        let (active, ready) = limiter();
        if let Ok(mut count) = active.lock() {
            *count = count.saturating_sub(1);
            ready.notify_one();
        }
    }
}
fn limiter() -> &'static (Mutex<usize>, Condvar) {
    static ACTIVE: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();
    ACTIVE.get_or_init(|| (Mutex::new(0), Condvar::new()))
}
