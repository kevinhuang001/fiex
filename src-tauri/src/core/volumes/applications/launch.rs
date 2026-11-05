use super::{platform, state::ApplicationSource};

pub fn open(uri: &str) -> Result<(), String> {
    let app = super::state::source(uri)?;
    match app.source {
        ApplicationSource::Imported { .. } => {
            let path = super::state::asset_path(uri)?;
            if !super::catalog().is_user_application(&path) {
                return Err(crate::i18n::t("无法启动应用").into());
            }
            platform::open_stored(&path)
        }
        ApplicationSource::System { stable_locator } => {
            super::catalog().open_system_application(&stable_locator)
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) use fiex_applications::is_executable_target;
