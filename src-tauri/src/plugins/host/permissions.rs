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
    let required = activation_requirements(&manifest)?;
    if enabled
        && !crate::plugin_permissions::activation_complete(
            &crate::plugin_permissions::status(id, &required.permissions)?,
            &required,
        )
    {
        return Err("review and approve every requested permission before activation".into());
    }
    crate::plugin_permissions::set_enabled(id, &required.permissions, enabled)?;
    global().stop(id);
    Ok(())
}
pub fn activate(
    id: &str,
    consent: &crate::plugin_permissions::ActivationConsent,
) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::activate(id, &activation_requirements(&manifest)?, consent)?;
    global().stop(id);
    Ok(())
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
    global().stop(id);
    Ok(())
}
pub fn set_network_domain(id: &str, domain: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_domain(id, &declared(&manifest)?, domain, grant)?;
    global().stop(id);
    Ok(())
}
pub fn set_command(id: &str, command: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_command(id, &declared(&manifest)?, command, grant)?;
    global().stop(id);
    Ok(())
}
pub fn set_permission(id: &str, permission: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_granted(id, &declared(&manifest)?, permission, grant)?;
    global().stop(id);
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
