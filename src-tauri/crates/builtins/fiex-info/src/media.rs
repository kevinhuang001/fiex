use crate::Localization;
use fiex_provider_contracts::{InfoFieldKey, InfoFieldResult};
use std::{fs, path::Path, time::Duration};

struct MediaRow {
    label: String,
    value: String,
}

struct MediaSection {
    id: String,
    rows: Vec<MediaRow>,
}

fn row(label: impl Into<String>, value: impl Into<String>) -> MediaRow {
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

fn audio_section(path: &Path, localization: &Localization) -> Option<MediaSection> {
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

fn mp4_section(path: &Path, localization: &Localization) -> Option<MediaSection> {
    use mp4::{Mp4Reader, TrackType};
    let file = fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mp4 = Mp4Reader::read_header(std::io::BufReader::new(file), size).ok()?;
    let mut rows = vec![row(
        localization.text("时长"),
        duration_text(mp4.duration()),
    )];
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
                audio = Some(
                    localization.format(
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
                    ),
                );
            }
            _ => {}
        }
    }
    if let Some(value) = video {
        rows.push(row(localization.text("视频"), value));
    }
    if let Some(value) = audio {
        rows.push(row(localization.text("音频"), value));
    }
    rows.push(row(
        localization.text("容器"),
        mp4.major_brand().to_string(),
    ));
    Some(MediaSection {
        id: "video".into(),
        rows,
    })
}

fn matroska_section(path: &Path, localization: &Localization) -> Option<MediaSection> {
    use matroska::Settings;
    let mkv = matroska::open(path).ok()?;
    let mut rows = Vec::new();
    if let Some(title) = mkv.info.title.filter(|value| !value.is_empty()) {
        rows.push(row(localization.text("标题"), title));
    }
    if let Some(duration) = mkv.info.duration {
        rows.push(row(localization.text("时长"), duration_text(duration)));
    }
    for track in &mkv.tracks {
        match &track.settings {
            Settings::Video(video) => rows.push(row(
                localization.text("视频"),
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
                localization.text("音频"),
                localization.format(
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
    rows.push(row(localization.text("容器"), "Matroska"));
    (!rows.is_empty()).then(|| MediaSection {
        id: "video".into(),
        rows,
    })
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
            if let Some(section) = matroska_section(path, localization) {
                sections.push(section);
            }
        } else if let Some(section) = mp4_section(path, localization) {
            sections.push(section);
        }
    } else if let Some(section) = audio_section(path, localization) {
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
