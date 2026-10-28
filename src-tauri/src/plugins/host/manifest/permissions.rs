use super::*;
use std::collections::BTreeSet;

pub fn declared_permissions(manifest: &PluginManifest) -> Result<BTreeSet<String>, String> {
    validate_extensions(&manifest.extensions)?;
    let mut declared = BTreeSet::new();
    let folders = &manifest.permissions.folders;
    let files = &manifest.permissions.files;
    if manifest.permissions.metadata.enabled {
        declared.insert("metadata".to_string());
    }
    if folders.read.enabled {
        declared.insert("folders.read".to_string());
    }
    for (name, requested) in [
        ("folders.create", folders.create.enabled),
        ("folders.delete", folders.delete.enabled),
        ("folders.rename", folders.rename.enabled),
    ] {
        if requested {
            declared.insert(name.into());
        }
    }
    if files.read.enabled {
        declared.insert("files.read".to_string());
    }
    if files.write.enabled {
        declared.insert("files.write".to_string());
    }
    for (name, requested) in [
        ("files.create", files.create.enabled),
        ("files.delete", files.delete.enabled),
        ("files.rename", files.rename.enabled),
    ] {
        if requested {
            declared.insert(name.into());
        }
    }
    if manifest.permissions.network.enabled
        || manifest.permissions.network.all
        || !manifest.permissions.network.domains.is_empty()
    {
        declared.insert("network".to_string());
    }
    if manifest.permissions.commands.enabled {
        declared.insert("commands".to_string());
    }
    if manifest.permissions.hooks.enabled {
        declared.insert("hooks".to_string());
    }
    crate::plugins::security::permissions::validate_commands(
        &declared,
        &manifest.permissions.commands.allow,
    )?;
    crate::plugins::security::permissions::validate_network_domains(
        &declared,
        &manifest.permissions.network.domains,
    )?;
    Ok(declared)
}

pub(crate) fn declared(manifest: &PluginManifest) -> Result<BTreeSet<String>, String> {
    declared_permissions(manifest)
}

pub fn activation_requirements(
    manifest: &PluginManifest,
) -> Result<crate::plugins::security::permissions::ActivationConsent, String> {
    let permissions = declared(manifest)?;
    let domains = crate::plugins::security::permissions::validate_network_domains(
        &permissions,
        &manifest.permissions.network.domains,
    )?;
    let commands = crate::plugins::security::permissions::validate_commands(
        &permissions,
        &manifest.permissions.commands.allow,
    )?;
    Ok(crate::plugins::security::permissions::ActivationConsent {
        permissions,
        network_all: manifest.permissions.network.all,
        domains,
        commands,
    })
}
