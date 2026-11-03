//! Core hooks use the same typed registration, matching, and owner binding as
//! other native provider kinds.
use super::RecentHook;
use crate::{
    infrastructure::provider_storage::CoreOwnerKey,
    plugins::{events::Resolver, provider_registry::native::NativeProviderRegistry},
};
use fiex_provider_contracts::{HookProvider, ProviderKind, ProviderMatchRule};
use std::sync::{Arc, OnceLock};

fn registry() -> &'static NativeProviderRegistry<dyn HookProvider> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn HookProvider>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut registry: NativeProviderRegistry<dyn HookProvider> =
            NativeProviderRegistry::new(ProviderKind::Hook);
        registry
            .register_for_event(
                "core:recent.hook",
                "afterOpen",
                CoreOwnerKey::new("recent"),
                100,
                ProviderMatchRule::default(),
                Arc::new(RecentHook),
            )
            .expect("invalid Recent Hook registration");
        registry
    })
}

pub(super) fn candidates(
    resource: &crate::plugins::resource::resource_context::ResourceSnapshot,
    event: &str,
) -> Vec<Resolver> {
    let targets = crate::plugins::events::matching::match_resources(std::slice::from_ref(resource));
    registry().event_candidates(event, &targets, None, 1)
}

pub(super) fn provider(id: &str) -> Option<Arc<dyn HookProvider>> {
    registry().get_arc(id)
}

pub(super) fn invocation(
    method: impl Into<String>,
    params: serde_json::Value,
    targets: &[crate::plugins::resource::resource_context::ResourceSnapshot],
) -> Result<crate::plugins::native_invocation::NativeProviderInvocation, String> {
    registry().invocation("core:recent.hook", method, params, targets, None)
}

#[cfg(test)]
pub(super) fn owner(id: &str) -> Option<CoreOwnerKey> {
    registry().owner(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_hook_and_vfs_share_the_registry_bound_storage_owner() {
        let owner = owner("core:recent.hook").unwrap();
        assert_eq!(
            crate::core::volumes::native_vfs::owner_for_provider("core:recent"),
            Some(owner)
        );
        let invocation = registry()
            .invocation(
                "core:recent.hook",
                "hooks.invoke",
                serde_json::json!({"event":"afterOpen"}),
                &[],
                None,
            )
            .unwrap();
        let hook = crate::plugins::native_invocation::NativeProviderInvocation::storage(
            &invocation,
            "core:recent.hook",
            "owner-binding-test",
        )
        .unwrap();
        assert!(
            crate::plugins::native_invocation::NativeProviderInvocation::storage(
                &invocation,
                "core:other.hook",
                "owner-binding-test"
            )
            .is_err()
        );
        let vfs = crate::core::volumes::native_vfs::storage_for_provider(
            "core:recent",
            "owner-binding-test",
        )
        .unwrap();
        assert!(hook
            .compare_and_set("shared-key", None, Some(serde_json::json!("recent")))
            .unwrap());
        assert_eq!(
            vfs.get("shared-key").unwrap(),
            Some(serde_json::json!("recent"))
        );
        hook.delete("shared-key").unwrap();
    }
}
