use crate::Localization;
use fiex_provider_contracts::{InfoFieldKey, InfoFieldResult};
use std::{path::Path, time::Duration};

mod audio;
mod matroska;
mod mp4;

pub(super) struct MediaRow {
    label: String,
    value: String,
}

pub(super) struct MediaSection {
    id: String,
    rows: Vec<MediaRow>,
}

pub(super) fn row(label: impl Into<String>, value: impl Into<String>) -> MediaRow {
    MediaRow {
        label: label.into(),
        value: value.into(),
    }
}

pub fn duration_text(duration: Duration) -> String {
    let total = duration.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Probe Core-native audio/video metadata without invoking any plugin.
fn media_sections(path: &Path, localization: &Localization) -> Vec<MediaSection> {
    if !path.is_file() {
        return Vec::new();
    }
    let ext = extension(path);
    let is_video = ["mp4", "m4v", "mov", "mkv", "webm"].contains(&ext.as_str());
    let mut sections = Vec::new();
    if is_video {
        if ["mkv", "webm"].contains(&ext.as_str()) {
            if let Some(section) = matroska::matroska_section(path, localization) {
                sections.push(section);
            }
        } else if let Some(section) = mp4::mp4_section(path, localization) {
            sections.push(section);
        }
    } else if let Some(section) = audio::audio_section(path, localization) {
        sections.push(section);
    }
    sections
}

pub fn compute_media_fields(
    path: &Path,
    fields: &[InfoFieldKey],
    localization: &Localization,
) -> Vec<InfoFieldResult> {
    let sections = media_sections(path, localization);
    fields
        .iter()
        .map(|field| {
            let label = match (field.subsection_id.as_str(), field.field_id.as_str()) {
                ("audio", "format") => "格式",
                (_, "title") => "标题",
                (_, "artist") => "艺术家",
                (_, "album") => "专辑",
                (_, "genre") => "流派",
                (_, "year") => "年份",
                (_, "track") => "音轨",
                (_, "disk") => "碟片",
                (_, "duration") => "时长",
                (_, "bitrate") => "码率",
                (_, "sampleRate") => "采样率",
                (_, "channels") => "声道",
                (_, "video") => "视频",
                (_, "audio") => "音频",
                (_, "container") => "容器",
                _ => "",
            };
            let value = sections
                .iter()
                .find(|section| section.id == field.subsection_id)
                .and_then(|section| {
                    section
                        .rows
                        .iter()
                        .find(|row| row.label == localization.text(label))
                })
                .map(|row| row.value.clone());
            InfoFieldResult {
                subsection_id: field.subsection_id.clone(),
                field_id: field.field_id.clone(),
                unavailable: value.is_none(),
                value,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::duration_text;
    use std::time::Duration;

    #[test]
    fn formats_media_durations_with_optional_hours() {
        assert_eq!(duration_text(Duration::from_secs(0)), "0:00");
        assert_eq!(duration_text(Duration::from_secs(59)), "0:59");
        assert_eq!(duration_text(Duration::from_secs(60)), "1:00");
        assert_eq!(duration_text(Duration::from_secs(3_661)), "1:01:01");
    }
}
