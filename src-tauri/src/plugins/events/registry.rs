use super::*;
use fiex_provider_contracts::{ProviderInput, ResourceEventRequest};

#[derive(Clone)]
pub struct Resolver {
    pub id: String,
    pub name: String,
    pub priority: i64,
    /// Authenticated plugin metadata for policy and presentation. Native
    /// candidates carry no plugin owner; executable instances live in each
    /// feature's typed ProviderInstances registry.
    pub plugin: Option<Box<crate::plugins::host::PluginInfo>>,
}
impl Resolver {
    pub fn builtin(id: &'static str, priority: i64) -> Self {
        Self {
            id: id.into(),
            name: "Fiex".into(),
            priority,
            plugin: None,
        }
    }
    pub fn json(&self) -> Value {
        json!({"id":self.id,"provider":self.name,"priority":self.priority})
    }
}

pub(super) fn bound_plugin_input(
    plugin_id: &str,
    registered_id: &str,
    input: &ProviderInput,
) -> Result<ProviderInput, String> {
    let mut input = input.clone();
    input.set_provider_id(plugin_provider_id(plugin_id, registered_id)?)?;
    Ok(input)
}

fn plugin_provider_id(plugin_id: &str, registered_id: &str) -> Result<Option<String>, String> {
    let prefix = format!("plugin:{plugin_id}:");
    if let Some(id) = registered_id.strip_prefix(&prefix) {
        if id.is_empty() {
            return Err("empty registered provider instance ID".into());
        }
        return Ok(Some(id.to_owned()));
    }
    if registered_id == format!("plugin:{plugin_id}") {
        return Ok(None);
    }
    // Never execute an endpoint with an identity from a different plugin.
    Err("provider instance does not belong to selected plugin".into())
}

#[cfg(test)]
mod instance_tests {
    use super::*;
    #[test]
    fn core_binds_selected_instance_and_erases_forged_provider_ids() {
        let input = ProviderInput::Resource(ResourceEventRequest {
            provider_id: Some("forged".into()),
        });
        let named = bound_plugin_input(
            "org.example.viewer",
            "plugin:org.example.viewer:media",
            &input,
        )
        .unwrap();
        assert_eq!(named.provider_id(), Some("media"));
        let single =
            bound_plugin_input("org.example.viewer", "plugin:org.example.viewer", &input).unwrap();
        assert_eq!(single.provider_id(), None);
        assert!(
            bound_plugin_input("org.example.viewer", "plugin:another.plugin:media", &input,)
                .is_err()
        );
        assert!(
            bound_plugin_input("org.example.viewer", "plugin:org.example.viewer:", &input,)
                .is_err()
        );
    }
}
