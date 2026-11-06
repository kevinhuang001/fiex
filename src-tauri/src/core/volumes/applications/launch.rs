use super::{platform, state::ApplicationSource};

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

#[cfg(target_os = "windows")]
pub(crate) use fiex_applications::is_executable_target;
