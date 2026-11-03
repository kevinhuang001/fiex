use super::{platform, state::ApplicationSource};
#[cfg(any(target_os = "windows", test))]
use std::path::Path;

pub fn open(uri: &str) -> Result<(), String> {
    let app = super::state::source(uri)?;
    match app.source {
        ApplicationSource::Imported { .. } => {
            let path = super::state::asset_path(uri)?;
            if !platform::is_user_application(&path) {
                return Err(crate::i18n::t("无法启动应用").into());
            }
            platform::open_stored(&path)
        }
        ApplicationSource::System { stable_locator } => {
            platform::platform_open(&super::encode(&stable_locator))
        }
    }
}

/// Executable file types that the Applications view is allowed to launch.
/// Documents and website shortcuts share the Start Menu namespace, so the
/// enumeration has to reject them explicitly.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[cfg(any(target_os = "windows", test))]
pub(crate) fn is_executable_target(value: &str) -> bool {
    if value.contains("://") {
        return false;
    }
    let extension = Path::new(value)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "exe" | "com" | "bat" | "cmd" | "msc" | "cpl" | "scr" | "ps1" | "appref-ms" | "jar"
    )
}
