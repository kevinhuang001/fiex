use super::*;
use fiex_provider_contracts::ProviderInvocation;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

type TimedProviderResult<T> = Option<(Resolver, Result<T, String>, Duration)>;

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
    /// Merge providers independently while preserving descriptor order.
    /// Calls backed by one plugin process share a serial lane; different plugin
    /// processes and native providers may run concurrently. The bounded worker
    /// count prevents a large catalog from creating unbounded threads.
    pub fn resolve_merge_parallel<P, T, R>(
        method: &str,
        providers: Vec<ProviderCandidate<P>>,
        invocation: &dyn ProviderInvocation,
        invoke: impl Fn(&Resolver, &P, &dyn ProviderInvocation) -> Result<T, String> + Sync,
        validate: impl FnMut(&Resolver, T) -> Result<T, String>,
        collect: impl FnOnce(Vec<(Resolver, Result<T, String>)>) -> R,
    ) -> R
    where
        P: ?Sized + Send + Sync,
        T: Send,
    {
        Self::resolve_merge_parallel_with_partial(
            method,
            providers,
            invocation,
            invoke,
            validate,
            |_, _| {},
            collect,
        )
    }

    /// Like `resolve_merge_parallel`, but reports each independently validated
    /// provider result as soon as it finishes. The final collector still sees
    /// deterministic descriptor order.
    pub fn resolve_merge_parallel_with_partial<P, T, R>(
        method: &str,
        providers: Vec<ProviderCandidate<P>>,
        invocation: &dyn ProviderInvocation,
        invoke: impl Fn(&Resolver, &P, &dyn ProviderInvocation) -> Result<T, String> + Sync,
        mut validate: impl FnMut(&Resolver, T) -> Result<T, String>,
        mut on_partial: impl FnMut(&Resolver, &Result<T, String>),
        collect: impl FnOnce(Vec<(Resolver, Result<T, String>)>) -> R,
    ) -> R
    where
        P: ?Sized + Send + Sync,
        T: Send,
    {
        let provider_count = providers.len();
        let mut groups: Vec<Vec<(usize, ProviderCandidate<P>)>> = Vec::new();
        let mut plugin_groups = HashMap::new();
        for (index, candidate) in providers.into_iter().enumerate() {
            let group = if let Some(plugin) = candidate.resolver.plugin.as_deref() {
                if let Some(group) = plugin_groups.get(&plugin.id) {
                    *group
                } else {
                    let group = groups.len();
                    groups.push(Vec::new());
                    plugin_groups.insert(plugin.id.clone(), group);
                    group
                }
            } else {
                let group = groups.len();
                groups.push(Vec::new());
                group
            };
            groups[group].push((index, candidate));
        }
        if groups.is_empty() {
            return collect(Vec::new());
        }

        let mut indexed: Vec<TimedProviderResult<T>> = std::iter::repeat_with(|| None)
            .take(provider_count)
            .collect();
        let trace = std::env::var_os("FIEX_TRACE_PROVIDER_PERF").is_some();
        static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
        let pool = POOL.get_or_init(|| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(8)
                .thread_name(|index| format!("provider-event-{index}"))
                .build()
                .expect("provider event pool configuration is valid")
        });
        let worker_panicked = std::thread::scope(|scope| {
            let (sender, receiver) = std::sync::mpsc::channel();
            let worker_sender = sender.clone();
            let group_lanes = &groups;
            let provider_invoke = &invoke;
            let worker = scope.spawn(move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    pool.install(|| {
                        group_lanes.par_iter().for_each(|group| {
                            let sender = worker_sender.clone();
                            for (index, candidate) in group {
                                let started = Instant::now();
                                let result = std::panic::catch_unwind(
                                    std::panic::AssertUnwindSafe(|| {
                                        require_event(method)?;
                                        if invocation.method() != method {
                                            return Err(
                                                "provider invocation method does not match routed event"
                                                    .into(),
                                            );
                                        }
                                        provider_invoke(
                                            &candidate.resolver,
                                            candidate.implementation.as_ref(),
                                            invocation,
                                        )
                                    }),
                                )
                                .unwrap_or_else(|_| Err("provider panicked".into()));
                                if sender
                                    .send((
                                        *index,
                                        candidate.resolver.clone(),
                                        result,
                                        started.elapsed(),
                                    ))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        });
                    })
                }))
                    .is_err()
            });
            drop(sender);

            for (index, resolver, result, elapsed) in receiver {
                if trace {
                    crate::infrastructure::logging::record_core(
                        "debug",
                        "provider.perf",
                        &format!(
                            "event={method} provider={} elapsed_ms={} status={}",
                            resolver.id,
                            elapsed.as_millis(),
                            if result.is_ok() { "ok" } else { "error" },
                        ),
                    );
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    result.and_then(|value| validate(&resolver, value))
                }))
                .unwrap_or_else(|_| Err("provider validation panicked".into()));
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    on_partial(&resolver, &result)
                }));
                indexed[index] = Some((resolver, result, elapsed));
            }

            worker.join().unwrap_or(true)
        });

        if worker_panicked {
            for group in &groups {
                for (index, candidate) in group {
                    if indexed[*index].is_some() {
                        continue;
                    }
                    let resolver = candidate.resolver.clone();
                    let result = Err("provider panicked".into());
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        on_partial(&resolver, &result)
                    }));
                    indexed[*index] = Some((resolver, result, Duration::ZERO));
                }
            }
        }

        let mut outcomes = Vec::with_capacity(provider_count);
        for result in indexed.into_iter().flatten() {
            let (resolver, result, _) = result;
            outcomes.push((resolver, result));
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
