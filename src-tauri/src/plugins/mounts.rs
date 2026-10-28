//! Core-owned navigation identities. URIs locate resources; they never grant access.

use crate::plugins::resource::fs_access::Scope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

mod uri;
pub use uri::*;

mod lifecycle;
pub use lifecycle::*;

#[cfg(test)]
mod tests;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    pub plugin_id: String,
    pub contribution_id: String,
    pub view_context: Value,
    pub resource_root: String,
}

#[derive(Clone)]
pub struct View {
    pub descriptor: Descriptor,
    pub scope: Scope,
    pub nodes: Arc<Mutex<HashMap<String, String>>>,
    created: Instant,
}

impl View {
    pub fn node_id(&self, relative: &str) -> Result<Option<String>, String> {
        self.nodes
            .lock()
            .map(|nodes| nodes.get(relative).cloned())
            .map_err(|_| "node index unavailable".into())
    }
}

#[derive(Default)]
struct Registry {
    views: HashMap<String, View>,
    references: HashMap<String, (HashSet<String>, HashSet<String>)>,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

/// The sole routing decision for Core navigation URIs. Cache-path recognition
/// remains an internal remote adapter detail, not a second navigation scheme.
pub enum Source {
    Connection(crate::remote::StoredConnection),
    View(View),
}

pub fn resolve(path: &str) -> Result<(Uri, Source), String> {
    let uri = parse(path)?;
    if let Some(conn) = crate::remote::connection_by_mount(&uri.mount_id) {
        crate::remote::ensure_provider_access(&conn)?;
        return Ok((uri, Source::Connection(conn)));
    }
    let view = view(&uri.mount_id)?;
    Ok((uri, Source::View(view)))
}
