//! Plugin-facing facade: logs and user-visible errors are separate sinks.
pub use crate::infrastructure::logging::{
    clear, list, report_log_limited, LogRecord as PluginError,
};
pub fn report(
    plugin_id: &str,
    source: &str,
    message: &str,
    detail: Option<&str>,
) -> Result<(), String> {
    crate::infrastructure::logging::report(plugin_id, source, message, detail)?;
    crate::infrastructure::errors::report(
        &format!("plugin:{plugin_id}"),
        "plugin.runtime",
        message,
        None,
        false,
    );
    Ok(())
}
pub fn report_limited(plugin_id: &str, code: &str, message: &str, detail: Option<&str>) {
    crate::infrastructure::logging::report_limited(plugin_id, code, message, detail);
    crate::infrastructure::errors::report(
        &format!("plugin:{plugin_id}"),
        &format!("plugin.{code}"),
        message,
        None,
        false,
    );
}
