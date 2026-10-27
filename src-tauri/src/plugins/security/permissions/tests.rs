use super::store::effective;
use super::*;
use std::collections::BTreeSet;
#[test]
fn activation_consent_can_only_narrow_the_declaration() {
    let required = ActivationConsent {
        permissions: [
            "files.read".into(),
            "files.write".into(),
            "commands".into(),
            "network".into(),
        ]
        .into(),
        network_all: false,
        commands: ["git".into(), "make".into()].into(),
        domains: ["example.com".into(), "api.example.com".into()].into(),
    };
    // Consent for everything and consent for a subset (including nothing) are
    // both valid: activation is partial.
    assert!(required.is_subset_of(&required));
    let partial = ActivationConsent {
        permissions: ["files.read".into(), "network".into()].into(),
        network_all: false,
        domains: ["api.example.com".into()].into(),
        commands: BTreeSet::new(),
    };
    assert!(partial.is_subset_of(&required));
    assert!(ActivationConsent::default().is_subset_of(&required));

    let mut extra = partial.clone();
    extra.permissions.insert("files.delete".into());
    assert!(!extra.is_subset_of(&required));
    let mut extra = partial.clone();
    extra.domains.insert("evil.example.com".into());
    assert!(!extra.is_subset_of(&required));
    let mut extra = partial.clone();
    extra.commands.insert("curl".into());
    assert!(!extra.is_subset_of(&required));

    // Targets cannot be separated from the capability that reaches them.
    let mut orphan_domain = partial.clone();
    orphan_domain.permissions = ["files.read".to_string()].into();
    assert!(!orphan_domain.is_subset_of(&required));
    let mut orphan_command = partial.clone();
    orphan_command.commands = ["git".to_string()].into();
    assert!(!orphan_command.is_subset_of(&required));

    // `networkAll` is only selectable when the manifest requested it.
    let mut network_all = partial.clone();
    network_all.network_all = true;
    assert!(!network_all.is_subset_of(&required));
    let requested_all = ActivationConsent {
        permissions: ["network".into()].into(),
        network_all: true,
        ..Default::default()
    };
    let mut selected_all = ActivationConsent {
        permissions: ["network".into()].into(),
        network_all: true,
        ..Default::default()
    };
    assert!(selected_all.is_subset_of(&requested_all));
    selected_all.network_all = false;
    assert!(selected_all.is_subset_of(&requested_all));
}

#[test]
fn activation_complete_tracks_the_selected_subset() {
    let required = ActivationConsent {
        permissions: [
            "files.read".into(),
            "files.write".into(),
            "commands".into(),
            "network".into(),
        ]
        .into(),
        network_all: false,
        commands: ["tool".into()].into(),
        domains: ["example.com".into()].into(),
    };
    let consent = ActivationConsent {
        permissions: ["files.read".into(), "commands".into(), "network".into()].into(),
        network_all: false,
        commands: ["tool".into()].into(),
        domains: ["example.com".into()].into(),
    };
    let mut grant = PluginGrant {
        approved: true,
        granted: ["files.read".to_string()].into(),
        ..Default::default()
    };
    // Only the selected items matter; `files.write` was deselected and is not
    // required for activation to be complete.
    assert!(!activation_complete(&grant, &consent));
    grant.granted = consent.permissions.clone();
    assert!(!activation_complete(&grant, &consent));
    grant.commands = consent.commands.clone();
    grant.network_domains = consent.domains.clone();
    assert!(activation_complete(&grant, &consent));
    assert!(!activation_complete(&grant, &required));
    grant.granted.remove("files.read");
    assert!(!activation_complete(&grant, &consent));
}

fn declared(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}
#[test]
fn all_plugins_start_unapproved() {
    let requested = declared(&["files.read", "files.write", "network"]);
    let state = effective(None, &requested);
    assert!(!state.approved && !state.enabled && state.granted.is_empty());
}
#[test]
fn removed_grants_stay_removed_and_new_scopes_never_auto_grant() {
    let old = declared(&["files.read"]);
    let current = PluginGrant {
        approved: true,
        enabled: true,
        granted: old,
        ..Default::default()
    };
    let new = declared(&["files.read", "files.write"]);
    let s = effective(Some(&current), &new);
    assert!(s.granted.contains("files.read") && !s.granted.contains("files.write"));
}
#[test]
fn network_all_requires_explicit_approval_and_disappears_if_undeclared() {
    let required = ActivationConsent {
        permissions: ["network".into()].into(),
        network_all: true,
        domains: BTreeSet::new(),
        commands: BTreeSet::new(),
    };
    let mut grant = PluginGrant {
        approved: true,
        enabled: true,
        granted: required.permissions.clone(),
        ..Default::default()
    };
    assert!(!activation_complete(&grant, &required));
    grant.network_all = true;
    assert!(activation_complete(&grant, &required));
    let hidden = effective(Some(&grant), &BTreeSet::new());
    assert!(!hidden.network_all);
    let kept = effective(Some(&grant), &required.permissions);
    assert!(kept.network_all);
}

#[test]
fn declaration_snapshot_bounds_runtime_grants() {
    let declared = declared(&["files.read"]);
    let approved = PluginGrant {
        approved: true,
        enabled: true,
        granted: declared.clone(),
        approved_declared: declared.clone(),
        ..Default::default()
    };
    assert!(declaration_approved(&approved, "files.read"));
    assert!(!declaration_approved(&approved, "files.write"));

    // A manifest that gained a permission without a new install approval
    // cannot turn it into a runtime grant.
    let mut widened = declared.clone();
    widened.insert("files.write".into());
    let state = effective(Some(&approved), &widened);
    assert!(state.granted.contains("files.read"));
    assert!(!declaration_approved(&state, "files.write"));
}

#[test]
fn command_grants_are_exact_and_require_declaration() {
    let commands = declared(&["commands"]);
    assert!(valid_command("git"));
    assert!(valid_command("python3"));
    for value in ["", "../git", "/usr/bin/git", "git --help", ".."] {
        assert!(!valid_command(value), "{value}");
    }
    let validated = validate_commands(&commands, &["git".to_string(), "Git".to_string()]).unwrap();
    assert_eq!(validated.into_iter().collect::<Vec<_>>(), vec!["git"]);
    assert!(validate_commands(&BTreeSet::new(), &["git".into()]).is_err());
    let grant = PluginGrant {
        approved: true,
        enabled: true,
        granted: commands.clone(),
        commands: ["git".to_string()].into(),
        ..Default::default()
    };
    assert_eq!(
        effective(Some(&grant), &commands)
            .commands
            .into_iter()
            .collect::<Vec<_>>(),
        vec!["git"]
    );
}

#[test]
fn domain_scopes_are_exact_and_require_explicit_grants() {
    let declared = declared(&["network"]);
    assert_eq!(normalize_domain("Example.COM").unwrap(), "example.com");
    for value in [
        "*.example.com",
        "https://example.com",
        "example.com:443",
        ".example.com",
        "evil/example.com",
        "",
    ] {
        assert!(normalize_domain(value).is_err(), "{value}");
    }
    assert!(validate_network_domains(&BTreeSet::new(), &["example.com".into()]).is_err());
    let grant = effective(
        Some(&PluginGrant {
            approved: true,
            enabled: true,
            granted: declared.clone(),
            network_domains: ["example.com".to_string()].into(),
            ..Default::default()
        }),
        &declared,
    );
    assert!(grant.network_domains.contains("example.com"));
    assert!(!grant.network_domains.contains("sub.example.com"));
    assert_eq!(normalize_domain("[::1]").unwrap(), "::1");
    assert!(is_loopback_domain("localhost"));
    assert!(is_loopback_domain("127.0.0.1"));
    assert!(is_loopback_domain("::1"));
    assert!(!is_loopback_domain("192.168.1.2"));
}
