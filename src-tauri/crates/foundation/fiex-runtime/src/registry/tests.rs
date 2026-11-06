#[cfg(test)]
mod unit_tests {
    use super::super::{ModuleDescriptor, RuntimeRegistry, TypedProviderRegistry};
    use fiex_provider_contracts::{ProviderKind, ProviderMatchRule};
    use fiex_storage::CoreOwnerKey;
    use std::sync::Arc;

    trait Handler: Send + Sync {
        fn call(&self) -> u32;
    }

    struct TestHandler(u32);

    impl Handler for TestHandler {
        fn call(&self) -> u32 {
            self.0
        }
    }

    #[test]
    fn module_registrar_shares_owner_and_keeps_provider_types() {
        let owner = CoreOwnerKey::new("recent");
        let mut hooks: TypedProviderRegistry<dyn Handler> =
            TypedProviderRegistry::new(ProviderKind::Hook);
        let module = ModuleDescriptor::new("core:recent", owner, "1");
        let mut registrar = hooks.module(module);
        registrar
            .register_for_event(
                "core:recent.hook",
                Some("afterOpen"),
                100,
                ProviderMatchRule::default(),
                Arc::new(TestHandler(7)),
            )
            .unwrap();

        let value = hooks.get("core:recent.hook").unwrap();
        assert_eq!(value.call(), 7);
        assert_eq!(hooks.owner("core:recent.hook"), Some(owner));
        assert!(hooks
            .event_candidates("beforeOpen", &[], None, 1)
            .is_empty());
        let candidates = hooks.event_candidates("afterOpen", &[], None, 1);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].module_id, "core:recent");
    }

    #[test]
    fn typed_registry_rejects_duplicate_ids_and_isolates_kinds() {
        let runtime = Arc::new(RuntimeRegistry::new());
        let mut hooks: TypedProviderRegistry<dyn Handler> =
            TypedProviderRegistry::from_runtime(Arc::clone(&runtime), ProviderKind::Hook);
        let mut info: TypedProviderRegistry<dyn Handler> =
            TypedProviderRegistry::from_runtime(runtime, ProviderKind::Info);
        hooks
            .register(
                "builtin:test",
                1,
                ProviderMatchRule::default(),
                Arc::new(TestHandler(1)),
            )
            .unwrap();
        assert!(hooks
            .register(
                "builtin:test",
                2,
                ProviderMatchRule::default(),
                Arc::new(TestHandler(2))
            )
            .is_err());
        info.register(
            "builtin:test",
            1,
            ProviderMatchRule::default(),
            Arc::new(TestHandler(3)),
        )
        .unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(info.len(), 1);
        assert_eq!(info.get("builtin:test").unwrap().call(), 3);
    }

    #[test]
    fn vfs_uri_routes_are_declared_by_registered_modules() {
        let mut vfs: TypedProviderRegistry<dyn Handler> =
            TypedProviderRegistry::new(ProviderKind::Vfs);
        {
            let mut module = vfs.module(ModuleDescriptor::new(
                "core:recent",
                CoreOwnerKey::new("recent"),
                "1",
            ));
            module
                .register(
                    "core:recent",
                    100,
                    ProviderMatchRule::default(),
                    Arc::new(TestHandler(1)),
                )
                .unwrap();
            module.register_vfs_uri("core:recent", "recent").unwrap();
            assert!(module
                .register_vfs_uri("core:recent", "bad/scheme")
                .is_err());
        }
        assert_eq!(vfs.route_uri("recent://item"), Some("core:recent"));
        assert_eq!(vfs.route_uri("unknown://item"), None);

        let mut second = vfs.module(ModuleDescriptor::new(
            "core:other",
            CoreOwnerKey::new("other"),
            "1",
        ));
        second
            .register(
                "core:other",
                100,
                ProviderMatchRule::default(),
                Arc::new(TestHandler(2)),
            )
            .unwrap();
        assert!(second.register_vfs_uri("core:other", "recent").is_err());
    }
}
