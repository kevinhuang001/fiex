//! Core-owned metadata and media Info calculations.
//!
//! This crate returns the shared Info DTOs and accepts localization callbacks.
//! Provider matching, resource authorization, partial-result delivery, and
//! caching remain in the application runtime.

pub use fiex_provider_contracts::{
    InfoCompute, InfoFieldDescriptor, InfoFieldKey, InfoFieldResult, InfoProviderPresentation,
    InfoSubsectionDescriptor, InfoSubsectionMatch, ResourceMetadata,
};

pub const MODULE_ID: &str = "core:info";

mod media;
mod media_descriptors;
mod metadata;
mod providers;

pub use media::{compute_media_fields, duration_text};
pub use media_descriptors::media_descriptors;
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
        let presentation = metadata_descriptors(
            "/tmp/notes.txt",
            true,
            None,
            Some(&projected),
            &localization(),
        );
        let basic = presentation
            .subsections
            .iter()
            .find(|subsection| subsection.id == "basic")
            .unwrap();
        assert!(basic.fields.iter().any(|field| field.id == "size"));
    }

    #[test]
    fn media_presentations_select_only_applicable_static_fields() {
        let audio = media_descriptors(
            std::path::Path::new("/virtual/song"),
            Some(&ResourceMetadata {
                name: "song.mp3".into(),
                kind: "file".into(),
                extension: Some("MP3".into()),
                ..Default::default()
            }),
            &localization(),
        );
        assert_eq!(audio.title, "媒体");
        assert_eq!(audio.subsections.len(), 1);
        assert_eq!(audio.subsections[0].id, "audio");
        assert!(
            audio.subsections[0]
                .fields
                .iter()
                .any(|field| field.id == "duration")
        );

        let video = media_descriptors(
            std::path::Path::new("/virtual/movie"),
            Some(&ResourceMetadata {
                name: "movie.mp4".into(),
                kind: "file".into(),
                mime: Some("video/mp4".into()),
                extension: Some("mp4".into()),
                ..Default::default()
            }),
            &localization(),
        );
        assert_eq!(video.subsections.len(), 1);
        assert_eq!(video.subsections[0].id, "video");
        assert!(
            video.subsections[0]
                .fields
                .iter()
                .any(|field| field.id == "video")
        );

        let mime_only = media_descriptors(
            std::path::Path::new("/virtual/unnamed"),
            Some(&ResourceMetadata {
                name: "unnamed".into(),
                kind: "file".into(),
                mime: Some("AUDIO/MPEG".into()),
                ..Default::default()
            }),
            &localization(),
        );
        assert_eq!(mime_only.subsections.len(), 1);
        assert_eq!(mime_only.subsections[0].id, "audio");
    }

    #[test]
    fn directory_statistics_compute_requested_fields_from_one_walk() {
        use std::sync::{Arc, atomic::AtomicBool};

        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("top.txt"), b"abc").unwrap();
        std::fs::create_dir(root.path().join("nested")).unwrap();
        std::fs::write(root.path().join("nested/inner.txt"), b"de").unwrap();
        let input = InfoMetadataInput {
            display: root.path().to_string_lossy().into_owned(),
            local: root.path().to_path_buf(),
            stat: Some(std::fs::metadata(root.path()).unwrap()),
            metadata: Some(ResourceMetadata {
                name: "root".into(),
                kind: "directory".into(),
                ..Default::default()
            }),
            is_local: true,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let fields = [
            InfoFieldKey {
                subsection_id: "directoryStats".into(),
                field_id: "directFiles".into(),
            },
            InfoFieldKey {
                subsection_id: "directoryStats".into(),
                field_id: "recursiveFiles".into(),
            },
            InfoFieldKey {
                subsection_id: "directoryStats".into(),
                field_id: "recursiveSize".into(),
            },
        ];
        let values = compute_metadata_fields(&input, &fields, &localization()).unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].value.as_deref(), Some("1"));
        assert_eq!(values[1].value.as_deref(), Some("2"));
        assert_eq!(values[2].value.as_deref(), Some("5 B (5 bytes)"));
    }
}
