use super::{row, InfoSection};
use std::{fs, path::Path, time::Duration};
pub(super) fn duration_text(duration: Duration) -> String {
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

fn audio_section(path: &Path) -> Option<InfoSection> {
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
                crate::i18n::t("标题"),
                tag.title().map(|value| value.to_string()),
            ),
            (
                crate::i18n::t("艺术家"),
                tag.artist().map(|value| value.to_string()),
            ),
            (
                crate::i18n::t("专辑"),
                tag.album().map(|value| value.to_string()),
            ),
            (
                crate::i18n::t("流派"),
                tag.genre().map(|value| value.to_string()),
            ),
            (
                crate::i18n::t("年份"),
                tag.date().map(|date| date.year.to_string()),
            ),
            (
                crate::i18n::t("音轨"),
                tag.track().map(|track| track.to_string()),
            ),
            (
                crate::i18n::t("碟片"),
                tag.disk().map(|disk| disk.to_string()),
            ),
        ] {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                rows.push(row(label, value));
            }
        }
    }
    rows.push(row(
        crate::i18n::t("时长"),
        duration_text(properties.duration()),
    ));
    if let Some(bitrate) = properties.audio_bitrate().filter(|bitrate| *bitrate > 0) {
        rows.push(row(crate::i18n::t("码率"), format!("{bitrate} kbps")));
    }
    if let Some(sample_rate) = properties.sample_rate().filter(|rate| *rate > 0) {
        rows.push(row(crate::i18n::t("采样率"), format!("{sample_rate} Hz")));
    }
    if let Some(channels) = properties.channels().filter(|channels| *channels > 0) {
        rows.push(row(crate::i18n::t("声道"), channels.to_string()));
    }
    if !rows.is_empty() {
        rows.insert(
            0,
            row(crate::i18n::t("格式"), format!("{:?}", tagged.file_type())),
        );
    }
    (!rows.is_empty()).then(|| InfoSection {
        id: "core.audio".into(),
        source: "core".into(),
        title: crate::i18n::t("音频").into(),
        plugin_id: None,
        provider: "Fiex".into(),
        rows,
    })
}

fn mp4_section(path: &Path) -> Option<InfoSection> {
    use mp4::{Mp4Reader, TrackType};
    let file = fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mp4 = Mp4Reader::read_header(std::io::BufReader::new(file), size).ok()?;
    let mut rows = vec![row(crate::i18n::t("时长"), duration_text(mp4.duration()))];
    let mut video = None;
    let mut audio = None;
    for track in mp4.tracks().values() {
        match track.track_type() {
            Ok(TrackType::Video) => {
                if video.is_none() {
                    video = Some(format!(
                        "{}×{} · {:.2} fps · {}",
                        track.width(),
                        track.height(),
                        track.frame_rate(),
                        track
                            .box_type()
                            .map(|value| value.to_string())
                            .unwrap_or_default()
                    ));
                }
            }
            Ok(TrackType::Audio) if audio.is_none() => {
                audio = Some(crate::i18n::format(
                    "{0} · {1} 声道",
                    &[
                        (
                            "0",
                            track
                                .box_type()
                                .map(|value| value.to_string())
                                .unwrap_or_default()
                                .to_string(),
                        ),
                        (
                            "1",
                            track
                                .channel_config()
                                .map(|value| value.to_string())
                                .unwrap_or_default()
                                .to_string(),
                        ),
                    ],
                ));
            }
            _ => {}
        }
    }
    if let Some(value) = video {
        rows.push(row(crate::i18n::t("视频"), value));
    }
    if let Some(value) = audio {
        rows.push(row(crate::i18n::t("音频"), value));
    }
    rows.push(row(crate::i18n::t("容器"), mp4.major_brand().to_string()));
    Some(InfoSection {
        id: "core.video.mp4".into(),
        source: "core".into(),
        title: crate::i18n::t("视频").into(),
        plugin_id: None,
        provider: "Fiex".into(),
        rows,
    })
}

fn matroska_section(path: &Path) -> Option<InfoSection> {
    use matroska::Settings;
    let mkv = matroska::open(path).ok()?;
    let mut rows = Vec::new();
    if let Some(title) = mkv.info.title.filter(|value| !value.is_empty()) {
        rows.push(row(crate::i18n::t("标题"), title));
    }
    if let Some(duration) = mkv.info.duration {
        rows.push(row(crate::i18n::t("时长"), duration_text(duration)));
    }
    for track in &mkv.tracks {
        match &track.settings {
            Settings::Video(video) => rows.push(row(
                crate::i18n::t("视频"),
                format!(
                    "{}×{} · {}",
                    video.pixel_width,
                    video.pixel_height,
                    track
                        .codec_name
                        .clone()
                        .unwrap_or_else(|| track.codec_id.clone())
                ),
            )),
            Settings::Audio(audio) => rows.push(row(
                crate::i18n::t("音频"),
                crate::i18n::format(
                    "{0} · {1} Hz · {2} 声道",
                    &[
                        (
                            "0",
                            track
                                .codec_name
                                .clone()
                                .unwrap_or_else(|| track.codec_id.clone())
                                .to_string(),
                        ),
                        ("1", format!("{}", audio.sample_rate)),
                        ("2", format!("{}", audio.channels)),
                    ],
                ),
            )),
            Settings::None => {}
        }
    }
    rows.push(row(crate::i18n::t("容器"), "Matroska"));
    (!rows.is_empty()).then(|| InfoSection {
        id: "core.video.matroska".into(),
        source: "core".into(),
        title: crate::i18n::t("视频").into(),
        plugin_id: None,
        provider: "Fiex".into(),
        rows,
    })
}

/// Probe Core-native audio/video metadata without invoking any plugin.
pub fn media_sections(path: &Path) -> Vec<InfoSection> {
    if !path.is_file() {
        return Vec::new();
    }
    let cache_key = crate::plugins::cache::key("info.media", path);
    if let Some(sections) = cache_key
        .as_deref()
        .and_then(crate::plugins::cache::get)
        .and_then(|value| serde_json::from_value(value).ok())
    {
        return sections;
    }
    let ext = extension(path);
    let is_video = ["mp4", "m4v", "mov", "mkv", "webm"].contains(&ext.as_str());
    let mut sections = Vec::new();
    if is_video {
        if ["mkv", "webm"].contains(&ext.as_str()) {
            if let Some(section) = matroska_section(path) {
                sections.push(section);
            }
        } else if let Some(section) = mp4_section(path) {
            sections.push(section);
        }
    } else if let Some(section) = audio_section(path) {
        sections.push(section);
    }
    if let Some(key) = cache_key {
        if let Ok(value) = serde_json::to_value(&sections) {
            crate::plugins::cache::put(key, value);
        }
    }
    sections
}
