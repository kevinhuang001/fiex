use super::*;

pub fn asset_path(uri: &str) -> Result<std::path::PathBuf, String> {
    if let Some(target) = pending_target(uri) {
        if target.kind != PendingKind::Application {
            return Err("folder has no application asset".into());
        }
        return fiex_applications::imported_asset_path(
            target
                .asset_id
                .as_deref()
                .ok_or("missing application asset ID")?,
            &target.extension,
        );
    }
    let app = source(uri)?;
    match app.source {
        ApplicationSource::Imported {
            asset_id,
            extension,
        } => fiex_applications::imported_asset_path(&asset_id, &extension),
        ApplicationSource::System { .. } => Err("system application has no private asset".into()),
    }
}
