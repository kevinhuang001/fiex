//! Resolve persisted resource mappings without persisting operation scopes.
use super::ResourceBroker;
use crate::plugins::provider_registry::FsOperation;

/// Resolve an untrusted persisted source locator for a user initiated Core
/// operation. Local file sources require the user to select the same file
/// again; a stored Recent mapping alone never grants filesystem access.
pub(crate) fn authorize_source(
    locator: &str,
    caller: &str,
    operation: FsOperation,
) -> Result<ResourceBroker, String> {
    if !matches!(caller, "core:export" | "core:preview" | "core:open") {
        return Err("NeedsAuthorization: mapped resource requires a user action".into());
    }
    if crate::core::fs::provider_registry::resource_access(locator).map_err(String::from)?
        == crate::core::fs::provider_registry::ResourceAccess::HostPath
    {
        #[cfg(feature = "desktop")]
        {
            let Some(selected) = rfd::FileDialog::new().pick_file() else {
                return Err("NeedsAuthorization: file authorization was cancelled".into());
            };
            let expected = dunce::canonicalize(locator).map_err(|error| error.to_string())?;
            let selected = dunce::canonicalize(selected).map_err(|error| error.to_string())?;
            if expected != selected {
                return Err(
                    "NeedsAuthorization: selected file does not match Recent source".into(),
                );
            }
        }
        #[cfg(not(feature = "desktop"))]
        {
            return Err(
                "NeedsAuthorization: local mapped access needs a user file selection".into(),
            );
        }
    }
    let broker = super::trusted(locator, caller)?;
    broker.require(operation, false).map_err(String::from)?;
    Ok(broker)
}
