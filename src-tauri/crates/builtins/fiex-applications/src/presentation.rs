//! Applications-specific presentation data projected from the virtual tree.

use crate::{app_id, app_uri_in, imported_asset_path, ApplicationCatalog, ApplicationSource};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationDecorationInfo {
    pub is_system: bool,
    pub icon_source: Option<String>,
}

impl ApplicationCatalog {
    /// Resolve icon and system-app presentation from the persisted tree.
    pub fn decoration_info(&self, uris: &[String]) -> HashMap<String, ApplicationDecorationInfo> {
        let Ok(tree) = self.store.snapshot() else {
            return HashMap::new();
        };
        let cached = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cached
            .clone()
            .unwrap_or_default();
        uris.iter()
            .filter_map(|uri| {
                let id = app_id(&tree, uri)?;
                let node = tree.apps.get(&id)?;
                if app_uri_in(&tree, &id).as_deref() != Some(uri) {
                    return None;
                }
                let (is_system, icon_source) = match &node.source {
                    ApplicationSource::System { .. } => (
                        true,
                        cached
                            .iter()
                            .find(|application| application.id == id)
                            .and_then(|application| application.icon_source.clone()),
                    ),
                    ApplicationSource::Imported {
                        asset_id,
                        extension,
                    } => (
                        false,
                        imported_asset_path(asset_id, extension)
                            .ok()
                            .map(|path| path.to_string_lossy().into_owned()),
                    ),
                };
                Some((
                    uri.clone(),
                    ApplicationDecorationInfo {
                        is_system,
                        icon_source,
                    },
                ))
            })
            .collect()
    }
}
