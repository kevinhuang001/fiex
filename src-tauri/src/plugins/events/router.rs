use super::*;
use fiex_provider_contracts::ProviderRequest;

pub struct EventRouter;
impl EventRouter {
    /// Dispatch already typed native and IPC provider implementations through
    /// the same ordering, panic boundary and owner/merge policy. Provider
    /// families keep their own shared Trait method and response DTO; this
    /// generic helper only owns event scheduling.
    pub fn resolve_typed<T, R>(
        method: &str,
        providers: Vec<Resolver>,
        request: &ProviderRequest,
        mut invoke: impl FnMut(&Resolver, &ProviderRequest) -> Result<T, String>,
        validate: impl FnMut(&Resolver, T) -> Result<T, String>,
        supported: impl FnMut(&T) -> bool,
        collect: impl FnOnce(Vec<(Resolver, Result<T, String>)>) -> R,
    ) -> R {
        Self::dispatch_resolvers(
            method,
            providers,
            |provider| {
                if request.method != method {
                    return Err("provider request method does not match routed event".into());
                }
                invoke(provider, request)
            },
            validate,
            supported,
            collect,
        )
    }

    fn dispatch_resolvers<I, T, R>(
        method: &str,
        providers: Vec<Resolver>,
        mut invoke: impl FnMut(&Resolver) -> Result<I, String>,
        mut validate: impl FnMut(&Resolver, I) -> Result<T, String>,
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
                let value = invoke(&provider)?;
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
