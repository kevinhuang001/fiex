//! Session-based Info descriptors and field computations.
use std::sync::OnceLock;

pub use fiex_info::{
    InfoCompute, InfoFieldDescriptor, InfoFieldKey, InfoFieldResult, InfoSectionDescriptor,
    timestamp_text,
};

mod native;
mod proxy;
mod sessions;
pub(crate) use sessions::{
    ComputeTrigger, InfoFieldDelta, InfoOpenResult, cancel_session_task, close_session,
    compute_fields, compute_session, initialize_session, mark_fields_error,
};

pub(crate) fn register_native(
    registry: &mut crate::plugins::provider_registry::native::NativeProviderRegistry<
        dyn fiex_provider_contracts::InfoProvider,
    >,
) {
    native::register(registry);
}

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
    sessions::forget_plugin(plugin_id);
}

fn localized_text(source: &'static str) -> String {
    crate::i18n::t(source).into()
}
fn localized_format(source: &'static str, values: &[(&str, String)]) -> String {
    crate::i18n::format(source, values)
}

pub(crate) fn localization() -> &'static fiex_info::Localization {
    static LOCALIZATION: OnceLock<fiex_info::Localization> = OnceLock::new();
    LOCALIZATION.get_or_init(|| fiex_info::Localization::new(localized_text, localized_format))
}

#[cfg(test)]
mod tests;
