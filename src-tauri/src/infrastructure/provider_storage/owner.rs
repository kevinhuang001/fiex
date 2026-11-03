//! Storage ownership is derived from the authenticated PluginHost session or
//! a Core Module registration. It is never accepted from plugin parameters.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OwnerId {
    Plugin(String),
    Core(CoreModule),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreModule {
    Applications,
    Recent,
}

impl CoreModule {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Applications => "applications",
            Self::Recent => "recent",
        }
    }
}

impl OwnerId {
    pub(super) fn plugin(id: &str) -> Self {
        Self::Plugin(id.to_owned())
    }

    pub(super) const fn core(module: CoreModule) -> Self {
        Self::Core(module)
    }

    pub(super) fn is_valid(&self) -> bool {
        match self {
            Self::Plugin(id) => valid_plugin_id(id),
            Self::Core(_) => true,
        }
    }
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
