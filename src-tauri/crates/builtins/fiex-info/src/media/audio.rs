use super::{MediaSection, duration_text, extension, row};
use crate::Localization;
use std::path::Path;

pub(super) fn audio_section(path: &Path, localization: &Localization) -> Option<MediaSection> {
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::tag::Accessor;
    const AUDIO: &[&str] = &[
        "mp3", "flac", "m4a", "aac", "ogg", "oga", "opus", "wav", "wave", "aiff", "aif", "ape",
        "wv", "wma",
    ];
    if !AUDIO.contains(&extension(path).as_str()) {
        return None;
    }
    let tagged = lofty::read_from_path(path).ok()?;
    let properties = tagged.properties();
    let mut rows = Vec::new();
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        for (label, value) in [
            (
                localization.text("标题"),
                tag.title().map(|value| value.to_string()),
            ),
            (
                localization.text("艺术家"),
                tag.artist().map(|value| value.to_string()),
            ),
            (
                localization.text("专辑"),
                tag.album().map(|value| value.to_string()),
            ),
            (
                localization.text("流派"),
                tag.genre().map(|value| value.to_string()),
            ),
            (
                localization.text("年份"),
                tag.date().map(|date| date.year.to_string()),
            ),
            (
                localization.text("音轨"),
                tag.track().map(|track| track.to_string()),
            ),
            (
                localization.text("碟片"),
                tag.disk().map(|disk| disk.to_string()),
            ),
        ] {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                rows.push(row(label, value));
            }
        }
    }
    rows.push(row(
        localization.text("时长"),
        duration_text(properties.duration()),
    ));
    if let Some(bitrate) = properties.audio_bitrate().filter(|bitrate| *bitrate > 0) {
        rows.push(row(localization.text("码率"), format!("{bitrate} kbps")));
    }
    if let Some(sample_rate) = properties.sample_rate().filter(|rate| *rate > 0) {
        rows.push(row(
            localization.text("采样率"),
            format!("{sample_rate} Hz"),
        ));
    }
    if let Some(channels) = properties.channels().filter(|channels| *channels > 0) {
        rows.push(row(localization.text("声道"), channels.to_string()));
    }
    if !rows.is_empty() {
        rows.insert(
            0,
            row(
                localization.text("格式"),
                format!("{:?}", tagged.file_type()),
            ),
        );
    }
    (!rows.is_empty()).then(|| MediaSection {
        id: "audio".into(),
        rows,
    })
}
