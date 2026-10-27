use super::Running;
use crate::plugins::security::policy::Capability;
use std::sync::atomic::Ordering;

impl Running {
    pub(super) fn policy(&self) -> crate::plugins::security::policy::PermissionPolicy<'_> {
        crate::plugins::security::policy::PermissionPolicy::new(&self.plugin_id, &self.declared)
    }

    fn refuse(
        &self,
        capability: crate::plugins::security::policy::Capability,
        decision: crate::plugins::security::policy::Decision,
    ) -> Result<(), String> {
        match decision {
            crate::plugins::security::policy::Decision::Allowed => Ok(()),
            crate::plugins::security::policy::Decision::Prompt => Err(format!(
                "PermissionDenied: {} was revoked for this operation",
                capability.as_str()
            )),
            crate::plugins::security::policy::Decision::Denied(reason) => {
                Err(reason.message(capability))
            }
        }
    }

    /// Ask at most once for this capability. The grant closes over the exact
    /// target for network/command prompts, so allowing one host or command
    /// never grants another.
    fn prompt(
        &self,
        capability: crate::plugins::security::policy::Capability,
        kind: crate::plugins::security::prompt::PromptKind,
        target: Option<&str>,
    ) -> Result<(), String> {
        use crate::plugins::security::prompt::PromptKind;
        if self.closed.load(Ordering::Acquire) {
            return Err("PermissionDenied: plugin process is stopping".into());
        }
        let plugin_id = self.plugin_id.clone();
        let grant_plugin = plugin_id.clone();
        let declared = self.declared.clone();
        let permission = capability.as_str().to_string();
        let grant_permission = permission.clone();
        let target = target.map(str::to_owned);
        let grant_target = target.clone();
        crate::plugins::security::prompt::request(
            &plugin_id,
            &permission,
            kind,
            target.as_deref(),
            Box::new(move || match kind {
                PromptKind::Permission => crate::plugins::security::permissions::set_granted(
                    &grant_plugin,
                    &declared,
                    &grant_permission,
                    true,
                )
                .map(|_| ()),
                PromptKind::Network => {
                    let target = grant_target
                        .as_deref()
                        .ok_or("missing network target for permission grant")?;
                    // Allow the base capability and this exact host together;
                    // without the base grant `domain_allowed` would still fail.
                    crate::plugins::security::permissions::set_granted(
                        &grant_plugin,
                        &declared,
                        "network",
                        true,
                    )?;
                    crate::plugins::security::permissions::set_domain(
                        &grant_plugin,
                        &declared,
                        target,
                        true,
                    )
                    .map(|_| ())
                }
                PromptKind::Command => {
                    let target = grant_target
                        .as_deref()
                        .ok_or("missing command for permission grant")?;
                    crate::plugins::security::permissions::set_granted(
                        &grant_plugin,
                        &declared,
                        "commands",
                        true,
                    )?;
                    crate::plugins::security::permissions::set_command(
                        &grant_plugin,
                        &declared,
                        target,
                        true,
                    )
                    .map(|_| ())
                }
            }),
        )
    }

    /// Manifest + user-state check. Callers must validate the operation scope
    /// first so an out-of-scope target is rejected without a prompt.
    pub(super) fn ensure_permission(
        &self,
        capability: crate::plugins::security::policy::Capability,
    ) -> Result<(), String> {
        match self.policy().check(capability)? {
            crate::plugins::security::policy::Decision::Allowed => Ok(()),
            crate::plugins::security::policy::Decision::Prompt => {
                self.prompt(
                    capability,
                    crate::plugins::security::prompt::PromptKind::Permission,
                    None,
                )?;
                // Re-check after the UI answer: a concurrent plugin disable or
                // grant-store failure must not let the operation through.
                let decision = self.policy().check(capability)?;
                self.refuse(capability, decision)
            }
            crate::plugins::security::policy::Decision::Denied(reason) => {
                Err(reason.message(capability))
            }
        }
    }

    /// Handle-based operations do not reopen a dialog. If the capability was
    /// revoked mid-operation the remaining call fails closed.
    pub(super) fn require_permission(
        &self,
        capability: crate::plugins::security::policy::Capability,
    ) -> Result<(), String> {
        let decision = self.policy().check(capability)?;
        self.refuse(capability, decision)
    }

    pub(super) fn require_writer(&self, owner: &str, handle: &str) -> Result<(), String> {
        let permission = crate::plugins::resource::fs_access::writer_permission(owner, handle)?;
        let capability = Capability::from_name(permission)
            .ok_or_else(|| "unknown write capability".to_string())?;
        self.require_permission(capability)
    }

    pub(super) fn ensure_fs(
        &self,
        scope: &crate::plugins::resource::fs_access::Scope,
        capability: crate::plugins::security::policy::Capability,
        path: &str,
    ) -> Result<(), String> {
        scope.authorize(path)?;
        self.ensure_permission(capability)
    }

    pub(super) fn ensure_network_target(&self, host: &str) -> Result<(), String> {
        use crate::plugins::security::policy::Capability;
        let host = crate::plugins::security::permissions::normalize_domain(host)?;
        match self.policy().domain_check(&host)? {
            crate::plugins::security::policy::Decision::Allowed => Ok(()),
            crate::plugins::security::policy::Decision::Prompt => {
                self.prompt(
                    Capability::Network,
                    crate::plugins::security::prompt::PromptKind::Network,
                    Some(&host),
                )?;
                match self.policy().domain_check(&host)? {
                    crate::plugins::security::policy::Decision::Allowed => Ok(()),
                    crate::plugins::security::policy::Decision::Prompt => {
                        Err("PermissionDenied: network target was not granted".into())
                    }
                    crate::plugins::security::policy::Decision::Denied(reason) => {
                        Err(reason.message(Capability::Network))
                    }
                }
            }
            crate::plugins::security::policy::Decision::Denied(reason) => {
                Err(reason.message(Capability::Network))
            }
        }
    }

    pub(super) fn ensure_command(&self, command: &str) -> Result<(), String> {
        use crate::plugins::security::policy::Capability;
        if !crate::plugins::security::permissions::valid_command(command) {
            return Err("invalid command name".into());
        }
        let command = command.to_ascii_lowercase();
        // Manifest `commands.allow` is the hard ceiling. A runtime grant for
        // another executable is not enough and cannot be requested.
        if !self.commands.contains(&command) {
            return Err("PermissionDenied: command is not declared by this plugin".into());
        }
        match self.policy().command_check(&command)? {
            crate::plugins::security::policy::Decision::Allowed => Ok(()),
            crate::plugins::security::policy::Decision::Prompt => {
                self.prompt(
                    Capability::Commands,
                    crate::plugins::security::prompt::PromptKind::Command,
                    Some(&command),
                )?;
                match self.policy().command_check(&command)? {
                    crate::plugins::security::policy::Decision::Allowed => Ok(()),
                    crate::plugins::security::policy::Decision::Prompt => {
                        Err("PermissionDenied: command was not granted".into())
                    }
                    crate::plugins::security::policy::Decision::Denied(reason) => {
                        Err(reason.message(Capability::Commands))
                    }
                }
            }
            crate::plugins::security::policy::Decision::Denied(reason) => {
                Err(reason.message(Capability::Commands))
            }
        }
    }
}
