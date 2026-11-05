use crate::Builtin;
use fiex_provider_contracts::{
    LocationMatch, PreviewProvider, PreviewView, ProviderError, ProviderInvocation,
    ProviderMatchExclude, ProviderMatchRule,
};
use std::sync::Arc;

pub trait PreviewModuleServices: Send + Sync {
    fn open(
        &self,
        kind: Builtin,
        ctx: &dyn ProviderInvocation,
    ) -> Result<PreviewView, ProviderError>;
}

pub struct PreviewProviderRegistration {
    pub id: &'static str,
    pub priority: i64,
    pub rule: ProviderMatchRule,
    pub provider: Arc<dyn PreviewProvider>,
}

pub struct PreviewModule {
    services: Arc<dyn PreviewModuleServices>,
}

impl PreviewModule {
    pub fn new(services: Arc<dyn PreviewModuleServices>) -> Self {
        Self { services }
    }

    pub fn registrations(&self) -> Vec<PreviewProviderRegistration> {
        [
            Builtin::Text,
            Builtin::Image,
            Builtin::Media,
            Builtin::Pdf,
            Builtin::Fallback,
        ]
        .into_iter()
        .map(|kind| PreviewProviderRegistration {
            id: kind.id(),
            priority: kind.priority(),
            rule: match_rule(kind),
            provider: Arc::new(BuiltinPreview {
                kind,
                services: self.services.clone(),
            }),
        })
        .collect()
    }
}

struct BuiltinPreview {
    kind: Builtin,
    services: Arc<dyn PreviewModuleServices>,
}

impl PreviewProvider for BuiltinPreview {
    fn open(&self, ctx: &dyn ProviderInvocation) -> Result<PreviewView, ProviderError> {
        self.services.open(self.kind, ctx)
    }
}

fn application_view_exclude() -> ProviderMatchExclude {
    ProviderMatchExclude {
        view: Some(LocationMatch {
            provider_ids: Some(vec!["core:applications".into()]),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn match_rule(kind: Builtin) -> ProviderMatchRule {
    let exclude = Some(application_view_exclude());
    match kind {
        Builtin::Text => ProviderMatchRule {
            mime_types: vec![
                "text/*".into(),
                "application/json".into(),
                "application/xml".into(),
                "application/javascript".into(),
            ],
            kinds: vec!["file".into()],
            exclude,
            ..Default::default()
        },
        Builtin::Image => ProviderMatchRule {
            mime_types: vec!["image/*".into()],
            kinds: vec!["file".into()],
            exclude,
            ..Default::default()
        },
        Builtin::Media => ProviderMatchRule {
            mime_types: vec!["audio/*".into(), "video/*".into()],
            kinds: vec!["file".into()],
            exclude,
            ..Default::default()
        },
        Builtin::Pdf => ProviderMatchRule {
            mime_types: vec!["application/pdf".into()],
            kinds: vec!["file".into()],
            exclude,
            ..Default::default()
        },
        Builtin::Fallback => ProviderMatchRule {
            exclude,
            ..Default::default()
        },
    }
}
