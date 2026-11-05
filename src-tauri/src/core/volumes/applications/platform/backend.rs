//! OS adapter implementing the Applications module's injected platform contract.
use super::super::{decode, encode, URI};
use super::{
    classify_host_application as classify_host, is_user_application as is_user_app, platform_list,
    platform_open,
};
use std::sync::Arc;

struct CoreApplicationsBackend;

pub(in crate::core::volumes::applications) fn backend(
) -> Arc<dyn fiex_applications::ApplicationsBackend> {
    Arc::new(CoreApplicationsBackend)
}

impl fiex_applications::ApplicationsBackend for CoreApplicationsBackend {
    fn enumerate_installed(&self) -> Result<Vec<fiex_applications::InstalledApplication>, String> {
        platform_list()?
            .into_iter()
            .map(|entry| {
                let encoded = entry
                    .path
                    .strip_prefix(URI)
                    .ok_or("platform application has an invalid identity")?;
                Ok(fiex_applications::InstalledApplication {
                    stable_locator: decode(encoded)?,
                    display_name: entry.name,
                    icon_source: entry.link_target,
                    extension: entry.extension,
                    modified: entry.modified,
                })
            })
            .collect()
    }

    fn open_system_application(&self, stable_locator: &str) -> Result<(), String> {
        platform_open(&encode(stable_locator))
    }

    fn classify_host_application(&self, path: &std::path::Path, target: Option<&str>) -> bool {
        classify_host(path, target)
    }

    fn is_user_application(&self, path: &std::path::Path) -> bool {
        is_user_app(path)
    }

    fn validate_import_asset(
        &self,
        path: &std::path::Path,
        display_name: &str,
    ) -> Result<(), String> {
        let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("Only regular application assets can be imported".into());
        }
        let extension = std::path::Path::new(display_name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let supported = match std::env::consts::OS {
            "windows" => {
                matches!(extension.as_str(), "exe" | "lnk")
                    && (extension == "lnk"
                        || crate::core::volumes::application_policy::executable_file(
                            &path.to_string_lossy(),
                        ))
            }
            "linux" => {
                matches!(extension.as_str(), "desktop" | "appimage")
                    && self.classify_host_application(path, None)
            }
            "macos" => false,
            _ => false,
        };
        if supported {
            Ok(())
        } else {
            Err("The file is not a supported application asset".into())
        }
    }
}
