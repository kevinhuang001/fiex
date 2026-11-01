use super::*;
use fiex_provider_contracts::ProviderRequest;
#[derive(Clone)]
pub enum Handler {
    Builtin(&'static str),
    Plugin(Box<crate::plugins::host::PluginInfo>),
}
#[derive(Clone)]
pub struct Resolver {
    pub id: String,
    pub name: String,
    pub priority: i64,
    pub handler: Handler,
}
impl Resolver {
    pub fn builtin(id: &'static str, priority: i64) -> Self {
        Self {
            id: id.into(),
            name: "Fiex".into(),
            priority,
            handler: Handler::Builtin(id),
        }
    }
    pub fn json(&self) -> Value {
        json!({"id":self.id,"provider":self.name,"priority":self.priority})
    }
}

/// Transport-sensitive provider identity binding happens during registry
/// selection, never inside the EventRouter dispatch/validation policy.
pub(super) fn bind_request(
    resolver: &Resolver,
    request: &ProviderRequest,
) -> Result<ProviderRequest, String> {
    match &resolver.handler {
        Handler::Builtin(_) => Ok(request.clone()),
        Handler::Plugin(plugin) => bound_plugin_request(request, &plugin.id, &resolver.id),
    }
}

fn bound_plugin_request(
    request: &ProviderRequest,
    plugin_id: &str,
    registered_id: &str,
) -> Result<ProviderRequest, String> {
    let mut routed = request.clone();
    let fields = routed
        .params
        .as_object_mut()
        .ok_or("provider parameters must be an object")?;
    fields.remove("providerId");
    let prefix = format!("plugin:{plugin_id}:");
    if let Some(id) = registered_id.strip_prefix(&prefix) {
        if id.is_empty() {
            return Err("empty registered provider instance ID".into());
        }
        fields.insert("providerId".into(), serde_json::json!(id));
    } else if registered_id != format!("plugin:{plugin_id}") {
        // Never execute an endpoint with an identity from a different plugin.
        return Err("provider instance does not belong to selected plugin".into());
    }
    Ok(routed)
}

#[cfg(test)]
mod instance_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn core_binds_selected_instance_and_erases_forged_provider_ids() {
        let forged = ProviderRequest {
            method: "info.resolve".into(),
            params: json!({"providerId":"forged","other":"ok"}),
        };
        let named = bound_plugin_request(
            &forged,
            "org.example.viewer",
            "plugin:org.example.viewer:media",
        )
        .unwrap();
        assert_eq!(named.params["providerId"], "media");
        assert_eq!(named.params["other"], "ok");
        let single =
            bound_plugin_request(&forged, "org.example.viewer", "plugin:org.example.viewer")
                .unwrap();
        assert!(single.params.get("providerId").is_none());
        assert!(
            bound_plugin_request(&forged, "org.example.viewer", "plugin:another.plugin:media")
                .is_err()
        );
        assert!(
            bound_plugin_request(&forged, "org.example.viewer", "plugin:org.example.viewer:")
                .is_err()
        );
        assert!(bound_plugin_request(
            &ProviderRequest {
                method: "info.resolve".into(),
                params: json!(null)
            },
            "org.example.viewer",
            "plugin:org.example.viewer"
        )
        .is_err());
    }
}
