//! Trusted storage owners. Provider IDs identify executable capabilities;
//! Core owner keys identify private state shared by those capabilities.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CoreOwnerKey(&'static str);

impl CoreOwnerKey {
    /// Core keys are selected by trusted registration code and stay a single
    /// path component so the on-disk namespace remains portable across OSes.
    pub(crate) fn new(value: &'static str) -> Self {
        assert!(valid_core_owner_key(value), "invalid Core owner key");
        Self(value)
    }

    pub(crate) const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

    pub(crate) fn is_core(&self, key: CoreOwnerKey) -> bool {
        matches!(self, Self::Core(owner) if *owner == key)
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
