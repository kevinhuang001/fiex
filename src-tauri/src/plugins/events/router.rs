use super::*;
pub struct EventRouter;
impl EventRouter {
    /// Every provider gets its own invocation/authorization. Validators admit
    /// one provider atomically; the feature supplies only its combination rule.
    pub fn resolve<T, R>(
        method: &str,
        providers: Vec<Resolver>,
        mut builtin: impl FnMut(&Resolver, &str) -> Result<Value, String>,
        mut plugin: impl FnMut(&crate::plugins::host::PluginInfo) -> Result<Value, String>,
        mut validate: impl FnMut(&Resolver, Value) -> Result<T, String>,
        mut supported: impl FnMut(&T) -> bool,
        collect: impl FnOnce(Vec<(Resolver, Result<T, String>)>) -> R,
    ) -> R {
        let dispatch = event(method)
            .map(|event| event.dispatch)
            .unwrap_or(Dispatch::Merge);
        let mut outcomes = Vec::new();
        for provider in providers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                require_event(method)?;
                let value = match &provider.handler {
                    Handler::Builtin(id) => builtin(&provider, id)?,
                    Handler::Plugin(p) => plugin(p)?,
                };
                validate_result(method, &value)?;
                validate(&provider, value)
            }))
            .unwrap_or_else(|_| Err("provider panicked".into()));
            let accepted = result.as_ref().is_ok_and(&mut supported);
            outcomes.push((provider, result));
            if matches!(dispatch, Dispatch::Owner)
                || (matches!(dispatch, Dispatch::FirstSupported) && accepted)
            {
                break;
            }
        }
        collect(outcomes)
    }
}
