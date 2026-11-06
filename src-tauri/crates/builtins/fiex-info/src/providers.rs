use crate::{media_sections, metadata_section, InfoSection, Localization};
use fiex_provider_contracts::{
    InfoProvider, ProviderError, ProviderInvocation, ProviderMatchRule, ResourceMetadata,
};
use std::{path::PathBuf, sync::Arc};

pub struct InfoMetadataInput {
    pub display: String,
    pub local: PathBuf,
    pub stat: Option<std::fs::Metadata>,
    pub metadata: Option<ResourceMetadata>,
}

pub trait InfoModuleServices: Send + Sync {
    fn metadata_input(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<InfoMetadataInput, ProviderError>;
    fn media_path(&self, ctx: &dyn ProviderInvocation) -> Result<PathBuf, ProviderError>;
    fn localization(&self) -> Localization;
}

pub struct InfoProviderRegistration {
    pub id: &'static str,
    pub priority: i64,
    pub rule: ProviderMatchRule,
    pub provider: Arc<dyn InfoProvider>,
}

pub struct InfoModule {
    services: Arc<dyn InfoModuleServices>,
}

impl InfoModule {
    pub fn new(services: Arc<dyn InfoModuleServices>) -> Self {
        Self { services }
    }

    pub fn registrations(&self) -> Vec<InfoProviderRegistration> {
        vec![
            InfoProviderRegistration {
                id: "core.metadata",
                priority: 1000,
                rule: ProviderMatchRule::default(),
                provider: Arc::new(MetadataInfo(self.services.clone())),
            },
            InfoProviderRegistration {
                id: "core.media",
                priority: 100,
                rule: ProviderMatchRule {
                    mime_types: vec!["audio/*".into(), "video/*".into()],
                    kinds: vec!["file".into()],
                    ..Default::default()
                },
                provider: Arc::new(MediaInfo(self.services.clone())),
            },
        ]
    }
}

struct MetadataInfo(Arc<dyn InfoModuleServices>);

impl InfoProvider for MetadataInfo {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<InfoSection>, ProviderError> {
        let input = self.0.metadata_input(ctx)?;
        Ok(vec![metadata_section(
            &input.display,
            &input.local,
            input.stat.as_ref(),
            input.metadata.as_ref(),
            &self.0.localization(),
        )])
    }
}

struct MediaInfo(Arc<dyn InfoModuleServices>);

impl InfoProvider for MediaInfo {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<InfoSection>, ProviderError> {
        let path = self.0.media_path(ctx)?;
        Ok(media_sections(&path, &self.0.localization()))
    }
}
