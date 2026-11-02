use super::*;
use fiex_provider_contracts::ProviderInvocation;
use std::sync::Arc;

/// A matched provider descriptor paired with its registered shared-contract
/// implementation. Native handlers and IPC proxies use the same router path.
pub struct ProviderCandidate<P: ?Sized + Send + Sync> {
    pub resolver: Resolver,
    pub implementation: Arc<P>,
}

impl<P: ?Sized + Send + Sync> Clone for ProviderCandidate<P> {
    fn clone(&self) -> Self {
        Self {
            resolver: self.resolver.clone(),
            implementation: Arc::clone(&self.implementation),
        }
    }
}

pub struct EventRouter;
impl EventRouter {
    /// Merge providers independently, retaining descriptor order for validation
    /// and collection. Bound concurrency so a large plugin catalog cannot
    /// create an unbounded number of threads for a single request.
    pub fn resolve_merge_parallel<P, T, R>(
        method: &str,
        providers: Vec<ProviderCandidate<P>>,
        invocation: &dyn ProviderInvocation,
        invoke: impl Fn(&Resolver, &P, &dyn ProviderInvocation) -> Result<T, String> + Sync,
        mut validate: impl FnMut(&Resolver, T) -> Result<T, String>,
        collect: impl FnOnce(Vec<(Resolver, Result<T, String>)>) -> R,
    ) -> R
    where
        P: ?Sized + Send + Sync,
        T: Send,
    {
        let mut outcomes = Vec::with_capacity(providers.len());
        for batch in providers.chunks(8) {
            let results = std::thread::scope(|scope| {
                batch
                    .iter()
                    .map(|candidate| {
                        let invoke = &invoke;
                        scope.spawn(move || {
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                require_event(method)?;
                                if invocation.method() != method {
                                    return Err("provider invocation method does not match routed event".into());
                                }
                                invoke(&candidate.resolver, candidate.implementation.as_ref(), invocation)
                            }))
                            .unwrap_or_else(|_| Err("provider panicked".into()))
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|handle| handle.join().unwrap_or_else(|_| Err("provider panicked".into())))
                    .collect::<Vec<_>>()
            });
            outcomes.extend(batch.iter().zip(results).map(|(candidate, result)| {
                let resolver = candidate.resolver.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    result.and_then(|value| validate(&resolver, value))
                }))
                .unwrap_or_else(|_| Err("provider panicked".into()));
                (resolver, result)
            }));
        }
        collect(outcomes)
    }
    /// Dispatch shared provider implementations through one scheduling and
    /// validation path. Transport selection happens while assembling the
    /// candidate; the router only calls the shared typed contract.
    pub fn resolve_shared<P, T, R>(
        method: &str,
        providers: Vec<ProviderCandidate<P>>,
        invocation: &dyn ProviderInvocation,
        mut invoke: impl FnMut(&Resolver, &P, &dyn ProviderInvocation) -> Result<T, String>,
        mut validate: impl FnMut(&Resolver, T) -> Result<T, String>,
        mut supported: impl FnMut(&T) -> bool,
        collect: impl FnOnce(Vec<(Resolver, Result<T, String>)>) -> R,
    ) -> R
    where
        P: ?Sized + Send + Sync,
    {
        let dispatch = event(method)
            .map(|event| event.dispatch)
            .unwrap_or(Dispatch::Merge);
        let mut outcomes = Vec::new();
        for candidate in providers {
            let resolver = candidate.resolver;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                require_event(method)?;
                if invocation.method() != method {
                    return Err("provider invocation method does not match routed event".into());
                }
                invoke(&resolver, candidate.implementation.as_ref(), invocation)
                    .and_then(|value| validate(&resolver, value))
            }))
            .unwrap_or_else(|_| Err("provider panicked".into()));
            let accepted = result.as_ref().is_ok_and(&mut supported);
            outcomes.push((resolver, result));
            if matches!(dispatch, Dispatch::Owner)
                || (matches!(dispatch, Dispatch::FirstSupported) && accepted)
            {
                break;
            }
        }
        collect(outcomes)
    }
}
