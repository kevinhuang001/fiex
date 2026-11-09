use crate::{
    InfoFieldKey, InfoFieldResult, InfoSectionDescriptor, Localization, compute_media_fields,
    compute_metadata_fields, media_descriptors, metadata_descriptors,
};
use fiex_provider_contracts::{
    InfoProvider, ProviderError, ProviderInvocation, ProviderMatchRule, ResourceMetadata,
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

pub struct InfoMetadataInput {
    pub display: String,
    pub local: PathBuf,
    pub stat: Option<std::fs::Metadata>,
    pub metadata: Option<ResourceMetadata>,
    pub is_local: bool,
    pub cancelled: Arc<AtomicBool>,
}

pub trait InfoModuleServices: Send + Sync {
    fn metadata_input(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<InfoMetadataInput, ProviderError>;
    fn media_descriptor_path(&self, ctx: &dyn ProviderInvocation)
    -> Result<PathBuf, ProviderError>;
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

    /// Build Core-owned field descriptors from the already-resolved resource
    /// snapshot. This path is synchronous and does not inspect file contents.
    pub fn sections_for(
        &self,
        provider_id: &str,
        ctx: &dyn ProviderInvocation,
    ) -> Result<Vec<InfoSectionDescriptor>, ProviderError> {
        match provider_id {
            "core.metadata" => {
                let input = self.services.metadata_input(ctx)?;
                Ok(metadata_descriptors(
                    &input.display,
                    input.is_local,
                    input.stat.as_ref(),
                    input.metadata.as_ref(),
                    &self.services.localization(),
                ))
            }
            "core.media" => {
                let path = self.services.media_descriptor_path(ctx)?;
                Ok(media_descriptors(&path, &self.services.localization()))
            }
            _ => Err(ProviderError::invalid_params("unknown Core Info provider")),
        }
    }
}

struct MetadataInfo(Arc<dyn InfoModuleServices>);

impl InfoProvider for MetadataInfo {
    fn compute(
        &self,
        ctx: &dyn ProviderInvocation,
        fields: &[InfoFieldKey],
    ) -> Result<Vec<InfoFieldResult>, ProviderError> {
        let input = self.0.metadata_input(ctx)?;
        compute_metadata_fields(&input, fields, &self.0.localization())
            .map_err(ProviderError::internal)
    }
}

struct MediaInfo(Arc<dyn InfoModuleServices>);

impl InfoProvider for MediaInfo {
    fn compute(
        &self,
        ctx: &dyn ProviderInvocation,
        fields: &[InfoFieldKey],
    ) -> Result<Vec<InfoFieldResult>, ProviderError> {
        let path = self.0.media_path(ctx)?;
        Ok(compute_media_fields(&path, fields, &self.0.localization()))
    }
}
