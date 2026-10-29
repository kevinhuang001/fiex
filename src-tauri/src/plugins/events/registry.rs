use super::*;
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

impl EventRouter {
    pub fn providers(
        method: &str,
        metadata: &[Value],
        directory: &str,
        mut providers: Vec<Resolver>,
    ) -> Vec<Resolver> {
        let Some(event) = event(method) else {
            return providers;
        };
        for plugin in crate::plugins::host::catalog_matching(|manifest| {
            matching::matches(method, &manifest.extensions, metadata, directory)
        }) {
            if !plugin.enabled || !plugin.approved || !plugin.backend_available {
                continue;
            }
            let Some(contribution) = plugin.extensions.get(event.extension) else {
                continue;
            };
            if !contribution.is_object() && contribution.as_bool() != Some(true) {
                continue;
            }
            let declared = plugin.declared_permissions.iter().cloned().collect();
            let policy =
                crate::plugins::security::policy::PermissionPolicy::new(&plugin.id, &declared);
            let required = match method {
                "decorations.resolve" => {
                    Some(crate::plugins::security::policy::Capability::FoldersRead)
                }
                "hooks.invoke" => Some(crate::plugins::security::policy::Capability::Hooks),
                _ => None,
            };
            if required.is_some_and(|capability| !policy.allowed(capability)) {
                continue;
            }
            providers.push(Resolver {
                id: format!("plugin:{}", plugin.id),
                name: plugin.name.clone(),
                priority: contribution
                    .get("priority")
                    .and_then(Value::as_i64)
                    .unwrap_or(50),
                handler: Handler::Plugin(Box::new(plugin)),
            });
        }
        providers.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(&b.id)));
        super::preferences::apply(event.extension, &mut providers);
        providers
    }
}
