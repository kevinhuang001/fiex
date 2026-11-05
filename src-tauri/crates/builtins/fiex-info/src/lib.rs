//! Core-owned metadata and media Info calculations.
//!
//! This crate returns the shared Info DTOs and accepts localization callbacks.
//! Provider matching, resource authorization, partial-result delivery, and
//! caching remain in the application runtime.

pub use fiex_provider_contracts::{InfoRow, InfoSection, ResourceMetadata};

mod media;
mod metadata;

pub use media::{duration_text, media_sections};
pub use metadata::{directory_size_with_cancel, metadata_section, timestamp_text};

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

fn row(label: impl Into<String>, value: impl Into<String>) -> InfoRow {
    InfoRow {
        label: label.into(),
        value: value.into(),
        copyable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

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
    fn metadata_projection_uses_shared_info_dtos() {
        let projected = ResourceMetadata {
            name: "notes.txt".into(),
            kind: "file".into(),
            extension: Some("txt".into()),
            size: Some(3),
            ..Default::default()
        };
        let section = metadata_section(
            "/tmp/notes.txt",
            std::path::Path::new("/tmp/notes.txt"),
            None,
            Some(&projected),
            &localization(),
        );
        assert_eq!(section.id, "core.metadata");
        assert!(section.rows.iter().any(|row| row.value == "3 B"));
    }

    #[test]
    fn directory_size_skips_symlinks_and_can_be_cancelled() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("file.bin"), [1, 2, 3]).unwrap();
        let nested = directory.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("more.bin"), [4, 5]).unwrap();
        let size = directory_size_with_cancel(directory.path(), &AtomicBool::new(false)).unwrap();
        assert_eq!(size, 5);
        assert!(directory_size_with_cancel(directory.path(), &AtomicBool::new(true)).is_err());
    }
}
