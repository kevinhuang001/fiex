//! Panel document lookup, CSP injection and asset serving.
use super::catalog::{canonical_under, discover};
use super::manifest::{activation_requirements, declared, PluginManifest};
use serde_json::Value;
use std::fs;
use std::path::{Component, Path};

/// The plugin document must define its own policy. It is served from the
/// asset origin rather than injected into the app shell, so the app's stricter
/// `script-src 'self'` does not block the plugin's inline bundle.
const PANEL_CSP: &str = "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'\">";
fn panel_document(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let insert = lower
        .find("<head")
        .and_then(|start| lower[start..].find('>').map(|offset| start + offset + 1));
    match insert {
        Some(index) => format!("{}{PANEL_CSP}{}", &html[..index], &html[index..]),
        None => format!("{PANEL_CSP}{html}"),
    }
}
/// Only static toolbar and preview registrations advertise panel ids.
/// Dynamic context menu panels are vetted as package assets at action time.
fn find_panel<'a>(manifest: &'a PluginManifest, panel_id: &str) -> Option<&'a Value> {
    if panel_id != manifest.id {
        return None;
    }
    ["toolbar", "preview"].iter().find_map(|key| {
        manifest
            .extensions
            .get(key)
            .and_then(Value::as_object)
            .and_then(|item| item.get("panel"))
    })
}
/// Host-approved precompiled, self-contained iframe HTML. Plugins cannot
/// supply absolute paths or read unrelated files through this API.
fn panel_source(plugin_id: &str, panel_id: &str) -> Result<String, String> {
    let (manifest, folder) = discover()
        .into_iter()
        .find(|(m, _)| m.id == plugin_id)
        .ok_or("plugin is not installed")?;
    let rights = crate::plugin_permissions::status(plugin_id, &declared(&manifest)?)?;
    if !rights.enabled
        || !crate::plugin_permissions::activation_complete(
            &rights,
            &activation_requirements(&manifest)?,
        )
    {
        return Err("plugin disabled or not approved".into());
    }
    let panel = find_panel(&manifest, panel_id).ok_or("plugin declares no such panel")?;
    let asset = panel
        .get("entry")
        .and_then(Value::as_str)
        .ok_or("panel entry missing")?;
    let rel = Path::new(asset);
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err("invalid panel resource path".into());
    }
    let file = folder.join(rel);
    if !canonical_under(&file, &folder) {
        return Err("panel resource outside plugin package".into());
    }
    let info = fs::metadata(&file).map_err(|e| e.to_string())?;
    if !info.is_file() || info.len() > 1024 * 1024 {
        return Err("panel HTML exceeds size limit".into());
    }
    fs::read_to_string(file).map_err(|e| e.to_string())
}
pub fn panel_html(plugin_id: &str, panel_id: &str) -> Result<String, String> {
    panel_source(plugin_id, panel_id)
}
/// Desktop panels are served from the asset protocol with their own CSP. A
/// `srcDoc` document inherits the app shell's CSP, which blocks inline plugin
/// bundles in packaged builds. The cache copy is content-addressed.
pub fn panel_asset(plugin_id: &str, panel_id: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let html = panel_document(&panel_source(plugin_id, panel_id)?);
    let digest = format!("{:x}", Sha256::digest(html.as_bytes()));
    let directory = dirs::cache_dir()
        .ok_or("cache directory unavailable")?
        .join("fiex")
        .join("plugin-panels")
        .join(plugin_id);
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let safe_panel = panel_id
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || value == '-' || value == '_' {
                value
            } else {
                '_'
            }
        })
        .collect::<String>();
    let path = directory.join(format!("{safe_panel}-{}.html", &digest[..16]));
    fs::write(&path, html).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Connection PluginView assets use the same CSP and canonical package checks
/// as existing panels; the entry is supplied by the native plugin result.
pub fn plugin_view_asset(plugin_id: &str, entry: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let (manifest, folder) = discover()
        .into_iter()
        .find(|(manifest, _)| manifest.id == plugin_id)
        .ok_or("plugin is not installed")?;
    let rights = crate::plugin_permissions::status(plugin_id, &declared(&manifest)?)?;
    if !rights.enabled
        || !crate::plugin_permissions::activation_complete(
            &rights,
            &activation_requirements(&manifest)?,
        )
    {
        return Err("plugin disabled or not approved".into());
    }
    let rel = Path::new(entry);
    if !rel
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("invalid pluginView package path".into());
    }
    let file = folder.join(rel);
    if !canonical_under(&file, &folder) {
        return Err("pluginView resource outside plugin package".into());
    }
    let metadata = fs::metadata(&file).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err("pluginView HTML exceeds size limit".into());
    }
    let html = panel_document(&fs::read_to_string(&file).map_err(|error| error.to_string())?);
    let digest = format!("{:x}", Sha256::digest(html.as_bytes()));
    let directory = dirs::cache_dir()
        .ok_or("cache directory unavailable")?
        .join("fiex")
        .join("plugin-views")
        .join(plugin_id);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let path = directory.join(format!("{}.html", &digest[..32]));
    fs::write(&path, html).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(extensions: Value) -> PluginManifest {
        serde_json::from_value(json!({
            "schemaVersion": 2,
            "id": "test.panels",
            "name": "Panels",
            "version": "1.0.0",
            "extensions": extensions
        }))
        .unwrap()
    }

    #[test]
    fn resolves_toolbar_panels_and_rejects_unknown_ids() {
        let manifest = manifest(json!({
            "toolbar": { "title": "Git", "panel": { "entry": "ui/git.html" } }
        }));
        assert_eq!(
            find_panel(&manifest, "test.panels").unwrap()["entry"],
            "ui/git.html"
        );
        assert!(find_panel(&manifest, "missing").is_none());
    }

    #[test]
    fn dynamic_menus_do_not_register_static_panel_ids() {
        let manifest = manifest(json!({"contextMenus": true}));
        assert!(find_panel(&manifest, "gpg.encrypt").is_none());
    }

    #[test]
    fn preview_panels_resolve_like_toolbar_panels() {
        let manifest: PluginManifest = serde_json::from_value(json!({
            "schemaVersion": 2,
            "id": "org.test.preview",
            "name": "Preview",
            "version": "0.1.0",
            "description": "fixture",
            "runtime": { "protocolVersion": "3.0", "entrypoints": {} },
            "permissions": {},
            "extensions": {
                "preview": {
                    "extensions": ["docx"],
                    "priority": 100,
                    "panel": { "entry": "ui/index.html" }
                }
            }
        }))
        .unwrap();
        let panel = find_panel(&manifest, "org.test.preview").expect("preview panel");
        assert_eq!(
            panel.get("entry").and_then(Value::as_str),
            Some("ui/index.html")
        );
        assert!(manifest.extensions["preview"].is_object());
    }

    #[test]
    fn panel_document_injects_the_plugin_csp_into_head() {
        let html = panel_document(
            "<!doctype html><html><head><title>Panel</title></head><body></body></html>",
        );
        assert!(html.contains(PANEL_CSP));
        let head = html.find("<head>").unwrap();
        let policy = html.find(PANEL_CSP).unwrap();
        let title = html.find("<title>").unwrap();
        assert!(
            head < policy && policy < title,
            "CSP must precede the bundle"
        );
        assert_eq!(html.matches(PANEL_CSP).count(), 1);
    }

    #[test]
    fn panel_document_prepends_the_csp_when_there_is_no_head() {
        let html = panel_document("<body>fragment</body>");
        assert!(html.starts_with(PANEL_CSP));
        assert!(html.ends_with("<body>fragment</body>"));
    }
}
