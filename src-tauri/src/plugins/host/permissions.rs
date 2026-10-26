//! Permission consent, activation and per-permission grant operations.
use super::catalog::discover;
use super::manifest::{activation_requirements, declared};
use super::process::global;
use std::collections::BTreeSet;

pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    let declared = declared(&manifest)?;
    // Enabling is package-level. A capability that the user switched off is
    // requested again when an operation actually needs it.
    crate::plugin_permissions::set_enabled(id, &declared, enabled)?;
    global().stop(id);
    Ok(())
}
pub fn activate(
    id: &str,
    consent: &crate::plugin_permissions::ActivationConsent,
) -> Result<(), String> {
    let (manifest, directory) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::activate(id, &activation_requirements(&manifest)?, consent)?;
    global().stop(id);
    start_backend(&manifest, &directory);
    Ok(())
}

/// Activation starts the native backend immediately instead of waiting for the
/// first invoke. Declarative plugins (themes) and packages without an
/// entrypoint for this platform have nothing to start. A launch failure is
/// recorded in the plugin error log: activation itself already succeeded and
/// is never rolled back, and the next invoke will retry.
fn start_backend(manifest: &super::manifest::PluginManifest, directory: &std::path::Path) {
    if crate::plugin_themes::is_theme(manifest)
        || super::catalog::executable(manifest, directory).is_err()
    {
        return;
    }
    if let Err(error) = global().start(&manifest.id) {
        let _ = crate::plugin_errors::report(
            &manifest.id,
            "host",
            &format!("plugin backend failed to start: {error}"),
            None,
        );
    }
}

pub fn declared_permissions_for(id: &str) -> Result<BTreeSet<String>, String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    declared(&manifest)
}

pub fn network_domains(id: &str) -> Result<Vec<String>, String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    let declared = declared(&manifest)?;
    Ok(crate::plugin_permissions::status(id, &declared)?
        .network_domains
        .into_iter()
        .collect())
}
pub fn permission_allowed(id: &str, permission: &str) -> Result<bool, String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    let declared = declared(&manifest)?;
    Ok(
        crate::plugin_permission_policy::PermissionPolicy::new(id, &declared)
            .allowed_name(permission),
    )
}
pub fn set_network_all(id: &str, all: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_network_all(id, &declared(&manifest)?, all)?;
    Ok(())
}
pub fn set_network_domain(id: &str, domain: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_domain(id, &declared(&manifest)?, domain, grant)?;
    if grant {
        crate::plugin_permission_prompt::clear_rejection(id, "network");
    }
    Ok(())
}
pub fn set_command(id: &str, command: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_command(id, &declared(&manifest)?, command, grant)?;
    if grant {
        crate::plugin_permission_prompt::clear_rejection(id, "commands");
    }
    Ok(())
}
pub fn set_permission(id: &str, permission: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_granted(id, &declared(&manifest)?, permission, grant)?;
    if grant {
        crate::plugin_permission_prompt::clear_rejection(id, permission);
    }
    Ok(())
}
pub fn approve_install(id: &str) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::approve_install_with_domains(
        id,
        &declared(&manifest)?,
        &manifest.permissions.network.domains,
        &manifest.permissions.commands.allow,
    )?;
    Ok(())
}
