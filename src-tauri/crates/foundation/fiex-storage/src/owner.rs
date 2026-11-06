//! Trusted storage owners. Provider IDs identify executable capabilities;
//! Core owner keys identify private state shared by those capabilities.

#[derive(Clone, Copy, Debug)]
pub struct CoreOwnerKey {
    name: &'static str,
    value_limit: Option<ScopedValueLimit>,
}

#[derive(Clone, Copy, Debug)]
struct ScopedValueLimit {
    scope: &'static str,
    key: &'static str,
    max_bytes: usize,
}

impl CoreOwnerKey {
    /// Core keys are selected by trusted registration code and stay a single
    /// path component so the on-disk namespace remains portable across OSes.
    pub fn new(value: &'static str) -> Self {
        assert!(valid_core_owner_key(value), "invalid Core owner key");
        Self {
            name: value,
            value_limit: None,
        }
    }

    /// Apply a module-owned value quota to one scope/key pair. The quota is
    /// supplied by the registering module; the storage engine does not know
    /// module IDs or business-specific persistence keys.
    pub fn with_value_limit(
        mut self,
        scope: &'static str,
        key: &'static str,
        max_bytes: usize,
    ) -> Self {
        self.value_limit = Some(ScopedValueLimit {
            scope,
            key,
            max_bytes,
        });
        self
    }

    pub const fn as_str(self) -> &'static str {
        self.name
    }

    pub(crate) fn max_value_bytes(self, scope: &str, key: &str, default: usize) -> usize {
        self.value_limit
            .filter(|limit| limit.scope == scope && limit.key == key)
            .map_or(default, |limit| limit.max_bytes)
    }
}

impl PartialEq for CoreOwnerKey {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for CoreOwnerKey {}

impl std::hash::Hash for CoreOwnerKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.name, state);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum OwnerId {
    Plugin(String),
    Core(CoreOwnerKey),
}

impl OwnerId {
    pub(crate) fn plugin(id: &str) -> Self {
        Self::Plugin(id.to_owned())
    }

    pub(crate) const fn core(key: CoreOwnerKey) -> Self {
        Self::Core(key)
    }

    pub(crate) fn is_valid(&self) -> bool {
        match self {
            Self::Plugin(id) => valid_plugin_id(id),
            Self::Core(key) => valid_core_owner_key(key.as_str()),
        }
    }
}

fn valid_core_owner_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        && value != "."
        && value != ".."
        && !value.starts_with('.')
}

fn valid_plugin_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        && value != "."
        && value != ".."
        && !value.starts_with('.')
}
