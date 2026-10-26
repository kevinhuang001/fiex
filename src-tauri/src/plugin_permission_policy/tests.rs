use super::capability::Capability;
use super::decision::{decide, decide_command, decide_domain, Decision, DenyReason};
use crate::plugin_permissions::PluginGrant;
use std::collections::BTreeSet;

fn declared(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn granted(values: &[&str]) -> PluginGrant {
    PluginGrant {
        approved: true,
        enabled: true,
        granted: declared(values),
        approved_declared: Some(declared(values)),
        ..Default::default()
    }
}

#[test]
fn capability_names_round_trip() {
    for capability in [
        Capability::Metadata,
        Capability::FilesRead,
        Capability::Network,
        Capability::Commands,
        Capability::Hooks,
    ] {
        assert_eq!(Capability::from_name(capability.as_str()), Some(capability));
    }
    assert_eq!(Capability::from_name("unknown"), None);
}

#[test]
fn decision_table_matches_the_three_layer_contract() {
    let files_read = declared(&["files.read"]);
    let allowed = granted(&["files.read"]);

    assert_eq!(
        decide(&files_read, &allowed, Capability::FilesRead),
        Decision::Allowed
    );

    let base = granted(&["files.read"]);
    let switched_off = PluginGrant {
        granted: BTreeSet::new(),
        ..base
    };
    assert_eq!(
        decide(&files_read, &switched_off, Capability::FilesRead),
        Decision::Prompt
    );

    assert_eq!(
        decide(&files_read, &switched_off, Capability::FilesWrite),
        Decision::Denied(DenyReason::NotDeclared)
    );

    let disabled = PluginGrant {
        enabled: false,
        ..allowed.clone()
    };
    assert_eq!(
        decide(&files_read, &disabled, Capability::FilesRead),
        Decision::Denied(DenyReason::Disabled)
    );

    let unapproved = PluginGrant {
        approved: false,
        ..allowed.clone()
    };
    assert_eq!(
        decide(&files_read, &unapproved, Capability::FilesRead),
        Decision::Denied(DenyReason::NotApproved)
    );

    // A manifest changed after install approval cannot be granted at
    // runtime, even if the user previously granted the same package.
    let widened = declared(&["files.read", "files.write"]);
    let snapshot = granted(&["files.read"]);
    assert_eq!(
        decide(&widened, &snapshot, Capability::FilesWrite),
        Decision::Denied(DenyReason::DeclarationNotApproved)
    );
}

#[test]
fn network_and_command_scopes_prompt_only_inside_the_granted_capability() {
    let network = declared(&["network"]);
    let mut grant = granted(&["network"]);
    grant.network_domains = ["example.com".to_string()].into();
    assert_eq!(
        decide_domain(&network, &grant, "example.com").unwrap(),
        Decision::Allowed
    );
    assert_eq!(
        decide_domain(&network, &grant, "other.example").unwrap(),
        Decision::Prompt
    );
    assert_eq!(
        decide_domain(&declared(&[]), &grant, "example.com").unwrap(),
        Decision::Denied(DenyReason::NotDeclared)
    );
    grant.network_domains.clear();
    assert_eq!(
        decide_domain(&network, &grant, "example.com").unwrap(),
        Decision::Prompt
    );

    let commands = declared(&["commands"]);
    let mut grant = granted(&["commands"]);
    grant.commands = ["git".to_string()].into();
    assert_eq!(decide_command(&commands, &grant, "Git"), Decision::Allowed);
    assert_eq!(decide_command(&commands, &grant, "bash"), Decision::Prompt);
}
