//! Core-owned metadata and media Info calculations.
//!
//! This crate returns the shared Info DTOs and accepts localization callbacks.
//! Provider matching, resource authorization, partial-result delivery, and
//! caching remain in the application runtime.

pub use fiex_provider_contracts::{
    InfoCompute, InfoFieldDescriptor, InfoFieldKey, InfoFieldResult, InfoSectionDescriptor,
    ResourceMetadata,
};

pub const MODULE_ID: &str = "core:info";

mod media;
mod metadata;
mod providers;

pub use media::{compute_media_fields, duration_text, media_descriptors};
pub use metadata::{compute_metadata_fields, metadata_descriptors, timestamp_text};
pub use providers::{InfoMetadataInput, InfoModule, InfoModuleServices, InfoProviderRegistration};

#[derive(Clone, Copy)]
pub struct Localization {
    text: fn(&'static str) -> String,
    format: fn(&'static str, &[(&str, String)]) -> String,
}

impl Localization {
    pub const fn new(
        text: fn(&'static str) -> String,
        format: fn(&'static str, &[(&str, String)]) -> String,
    ) -> Self {
        Self { text, format }
    }

    fn text(&self, key: &'static str) -> String {
        (self.text)(key)
    }

    fn format(&self, key: &'static str, values: &[(&str, String)]) -> String {
        (self.format)(key, values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(key: &'static str) -> String {
        key.into()
    }

    fn identity_format(key: &'static str, values: &[(&str, String)]) -> String {
        values.iter().fold(key.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
    }

    fn localization() -> Localization {
        Localization::new(identity, identity_format)
    }

    #[test]
    fn timestamps_are_formatted_in_utc() {
        assert_eq!(timestamp_text(0), "1970-01-01 00:00");
    }

    #[test]
    fn metadata_projection_uses_field_descriptors() {
        let projected = ResourceMetadata {
            name: "notes.txt".into(),
            kind: "file".into(),
            extension: Some("txt".into()),
            size: Some(3),
            ..Default::default()
        };
        let sections = metadata_descriptors(
            "/tmp/notes.txt",
            true,
            None,
            Some(&projected),
            &localization(),
        );
        let basic = sections
            .iter()
            .find(|section| section.id == "basic")
            .unwrap();
        assert!(basic.fields.iter().any(|field| field.id == "size"));
    }
}
