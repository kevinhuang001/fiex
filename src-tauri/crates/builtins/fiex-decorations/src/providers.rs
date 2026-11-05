use crate::NativeKind;
use fiex_provider_contracts::{
    DecorationContribution, DecorationProvider, LocationMatch, ProviderError, ProviderInvocation,
    ProviderMatchRule,
};
use std::sync::Arc;

pub trait DecorationModuleServices: Send + Sync {
    fn resolve(
        &self,
        kind: NativeKind,
        ctx: &dyn ProviderInvocation,
    ) -> Result<Vec<DecorationContribution>, ProviderError>;
}

pub struct DecorationProviderRegistration {
    pub id: &'static str,
    pub priority: i64,
    pub rule: ProviderMatchRule,
    pub provider: Arc<dyn DecorationProvider>,
}

pub struct DecorationModule {
    services: Arc<dyn DecorationModuleServices>,
}

impl DecorationModule {
    pub fn new(services: Arc<dyn DecorationModuleServices>) -> Self {
        Self { services }
    }

    pub fn registrations(&self) -> Vec<DecorationProviderRegistration> {
        [
            ("builtin:tags", -12, NativeKind::Tags),
            (
                "builtin:connection-status",
                -10,
                NativeKind::ConnectionStatus,
            ),
            ("builtin:application-icon", -9, NativeKind::ApplicationIcon),
        ]
        .into_iter()
        .map(|(id, priority, kind)| DecorationProviderRegistration {
            id,
            priority,
            rule: match_rule(kind),
            provider: Arc::new(BuiltinDecoration {
                kind,
                services: self.services.clone(),
            }),
        })
        .collect()
    }
}

struct BuiltinDecoration {
    kind: NativeKind,
    services: Arc<dyn DecorationModuleServices>,
}

impl DecorationProvider for BuiltinDecoration {
    fn resolve(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<Vec<DecorationContribution>, ProviderError> {
        self.services.resolve(self.kind, ctx)
    }
}

fn match_rule(kind: NativeKind) -> ProviderMatchRule {
    match kind {
        NativeKind::Tags => ProviderMatchRule {
            storage: Some(LocationMatch {
                types: Some(vec!["fs".into()]),
                ..Default::default()
            }),
            ..Default::default()
        },
        NativeKind::ConnectionStatus => ProviderMatchRule {
            storage: Some(LocationMatch {
                types: Some(vec!["vfs".into()]),
                ..Default::default()
            }),
            ..Default::default()
        },
        NativeKind::ApplicationIcon => ProviderMatchRule {
            view: Some(LocationMatch {
                provider_ids: Some(vec!["core:applications".into()]),
                ..Default::default()
            }),
            ..Default::default()
        },
    }
}
