//! "Show info" registry: built-in providers plus standard `info.resolve`.
//!
//! Built-in and plugin providers return the same section/row domain types.
//! Providers share the event request and bounded feature result validation.
use crate::plugin_resource_broker::Resource;
use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path, time::Duration};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoRow {
    pub label: String,
    pub value: String,
    /// Render a copy control next to this value in the Info dialog.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub copyable: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoSection {
    /// Stable section id (`core.metadata`, `<plugin>.<section>`).
    pub id: String,
    /// `core` for built-in probes, `plugin` for an extension contribution.
    pub source: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    pub provider: String,
    pub rows: Vec<InfoRow>,
}

fn row(label: impl Into<String>, value: impl Into<String>) -> InfoRow {
    InfoRow {
        label: label.into(),
        value: value.into(),
        copyable: false,
    }
}

fn duration_text(duration: Duration) -> String {
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
            ("标题", tag.title().map(|value| value.to_string())),
            ("艺术家", tag.artist().map(|value| value.to_string())),
            ("专辑", tag.album().map(|value| value.to_string())),
            ("流派", tag.genre().map(|value| value.to_string())),
            ("年份", tag.date().map(|date| date.year.to_string())),
            ("音轨", tag.track().map(|track| track.to_string())),
            ("碟片", tag.disk().map(|disk| disk.to_string())),
        ] {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                rows.push(row(label, value));
            }
        }
    }
    rows.push(row("时长", duration_text(properties.duration())));
    if let Some(bitrate) = properties.audio_bitrate().filter(|bitrate| *bitrate > 0) {
        rows.push(row("码率", format!("{bitrate} kbps")));
    }
    if let Some(sample_rate) = properties.sample_rate().filter(|rate| *rate > 0) {
        rows.push(row("采样率", format!("{sample_rate} Hz")));
    }
    if let Some(channels) = properties.channels().filter(|channels| *channels > 0) {
        rows.push(row("声道", channels.to_string()));
    }
    if !rows.is_empty() {
        rows.insert(0, row("格式", format!("{:?}", tagged.file_type())));
    }
    (!rows.is_empty()).then(|| InfoSection {
        id: "core.audio".into(),
        source: "core".into(),
        title: "音频".into(),
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
    let mut rows = vec![row("时长", duration_text(mp4.duration()))];
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
                audio = Some(format!(
                    "{} · {} 声道",
                    track
                        .box_type()
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                    track
                        .channel_config()
                        .map(|value| value.to_string())
                        .unwrap_or_default()
                ));
            }
            _ => {}
        }
    }
    if let Some(value) = video {
        rows.push(row("视频", value));
    }
    if let Some(value) = audio {
        rows.push(row("音频", value));
    }
    rows.push(row("容器", mp4.major_brand().to_string()));
    Some(InfoSection {
        id: "core.video.mp4".into(),
        source: "core".into(),
        title: "视频".into(),
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
        rows.push(row("标题", title));
    }
    if let Some(duration) = mkv.info.duration {
        rows.push(row("时长", duration_text(duration)));
    }
    for track in &mkv.tracks {
        match &track.settings {
            Settings::Video(video) => rows.push(row(
                "视频",
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
                "音频",
                format!(
                    "{} · {} Hz · {} 声道",
                    track
                        .codec_name
                        .clone()
                        .unwrap_or_else(|| track.codec_id.clone()),
                    audio.sample_rate,
                    audio.channels
                ),
            )),
            Settings::None => {}
        }
    }
    rows.push(row("容器", "Matroska"));
    (!rows.is_empty()).then(|| InfoSection {
        id: "core.video.matroska".into(),
        source: "core".into(),
        title: "视频".into(),
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
    sections
}

fn display_parent(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    trimmed
        .rsplit_once(['/', '\\'])
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn timestamp_text(seconds: i64) -> String {
    // UTC civil date without pulling a formatting crate into Core.
    let days = seconds.div_euclid(86_400);
    let remaining = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        remaining / 3600,
        (remaining % 3600) / 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn metadata_section(display: &str, _local: &Path, projected: Option<&Value>) -> InfoSection {
    let metadata = projected.cloned().unwrap_or(Value::Null);
    let is_dir = metadata["kind"] == "directory";
    let mut rows = vec![
        row("种类", if is_dir { "文件夹" } else { "文件" }),
        row(
            "大小",
            if is_dir {
                "—".into()
            } else {
                metadata["size"]
                    .as_u64()
                    .map(size_text)
                    .unwrap_or_else(|| "—".into())
            },
        ),
        row("位置", display_parent(display)),
    ];
    if let Some(ext) = metadata["extension"].as_str().filter(|s| !s.is_empty()) {
        rows.push(row("扩展名", ext));
    }
    if let Some(mime) = metadata["mime"].as_str() {
        rows.push(row("MIME", mime));
    }
    if let Some(modified) = metadata["modified"].as_i64() {
        rows.push(row("修改时间", timestamp_text(modified)));
    }
    if let Some(readonly) = metadata["readonly"].as_bool() {
        rows.push(row("权限", if readonly { "只读" } else { "可读写" }));
    }
    InfoSection {
        id: "core.metadata".into(),
        source: "core".into(),
        title: "文件".into(),
        plugin_id: None,
        provider: "Fiex".into(),
        rows,
    }
}

fn decode_sections(
    provider: &crate::plugin_events::Provider,
    value: Value,
) -> Result<Vec<InfoSection>, String> {
    let sections = value
        .get("sections")
        .and_then(Value::as_array)
        .filter(|s| s.len() <= 64)
        .ok_or("invalid info sections")?;
    let mut result = Vec::new();
    for section in sections {
        let text = |v: &Value| {
            v.as_str()
                .filter(|s| s.len() <= 16384)
                .map(str::to_owned)
                .ok_or("invalid info text")
        };
        let id = text(&section["id"])?;
        let title = text(&section["title"])?;
        let rows = section["rows"]
            .as_array()
            .filter(|r| r.len() <= 256)
            .ok_or("invalid info rows")?;
        let mut decoded = Vec::new();
        for row in rows {
            let copyable = match row.get("copyable") {
                None => false,
                Some(v) => v.as_bool().ok_or("invalid copyable")?,
            };
            decoded.push(InfoRow {
                label: text(&row["label"])?,
                value: text(&row["value"])?,
                copyable,
            });
        }
        result.push(InfoSection {
            id: match &provider.handler {
                crate::plugin_events::Handler::Plugin(plugin) => format!("{}.{}", plugin.id, id),
                _ => id,
            },
            source: if matches!(&provider.handler, crate::plugin_events::Handler::Plugin(_)) {
                "plugin"
            } else {
                "core"
            }
            .into(),
            title,
            plugin_id: match &provider.handler {
                crate::plugin_events::Handler::Plugin(p) => Some(p.id.clone()),
                _ => None,
            },
            provider: provider.name.clone(),
            rows: decoded,
        });
    }
    Ok(result)
}

/// Resolve all providers through the common registry and validation boundary.
pub fn file_info_at(display: &str, local: &str) -> Vec<InfoSection> {
    use crate::plugin_events::{self as events, Handler, Provider};
    let resource = Resource::new(display, Path::new(local));
    let mut builtin = vec![Provider::builtin("core.metadata", 1000)];
    if !resource.is_dir {
        builtin.push(Provider::builtin("core.media", 100));
    }
    let providers = events::registry(
        "info",
        &resource.ext,
        &resource.mime,
        resource.is_dir,
        builtin,
    );
    let mut sections = Vec::new();
    for provider in providers {
        let result = events::dispatch(
            &provider,
            |id| {
                let sections = match id {
                    "core.metadata" => vec![metadata_section(
                        display,
                        &resource.local,
                        Some(&resource.metadata),
                    )],
                    "core.media" => media_sections(&resource.local),
                    _ => return Err("unknown builtin info provider".into()),
                };
                Ok(serde_json::json!({"sections": sections}))
            },
            |plugin| {
                events::invoke(
                    &plugin.id,
                    "info.resolve",
                    resource.request(true, serde_json::json!({})),
                    &resource.local,
                )
            },
            |value| decode_sections(&provider, value),
        );
        match result {
            Ok(mut resolved) => sections.append(&mut resolved),
            Err(error) => sections.push(InfoSection {
                id: format!(
                    "{}.error",
                    provider.id.strip_prefix("plugin:").unwrap_or(&provider.id)
                ),
                source: if matches!(&provider.handler, Handler::Plugin(_)) {
                    "plugin"
                } else {
                    "core"
                }
                .into(),
                title: provider.name.clone(),
                plugin_id: match &provider.handler {
                    Handler::Plugin(p) => Some(p.id.clone()),
                    _ => None,
                },
                provider: provider.name,
                rows: vec![row("错误", error)],
            }),
        }
    }
    sections
}

/// Metadata-only remote Info uses the existing stat result, never materializes content.
pub fn metadata_info(display: &str, entry: &crate::files::Entry) -> Vec<InfoSection> {
    vec![metadata_section(
        display,
        Path::new(""),
        Some(&Resource::metadata_from_entry(entry)),
    )]
}

pub fn requires_content(entry: &crate::files::Entry) -> bool {
    if entry.is_dir {
        return false;
    }
    if entry.mime.starts_with("audio/") || entry.mime.starts_with("video/") {
        return true;
    }
    crate::plugin_events::registry("info", &entry.extension, &entry.mime, false, Vec::new())
        .iter()
        .any(|provider| {
            let crate::plugin_events::Handler::Plugin(plugin) = &provider.handler else {
                return false;
            };
            let declared = plugin.declared_permissions.iter().cloned().collect();
            crate::plugin_permission_policy::PermissionPolicy::new(&plugin.id, &declared)
                .allowed(crate::plugin_permission_policy::Capability::FilesRead)
        })
}

pub fn file_info(path: &str) -> Vec<InfoSection> {
    file_info_at(path, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_info_is_rejected_instead_of_partially_rendered() {
        let provider = crate::plugin_events::Provider::builtin("test", 0);
        assert!(decode_sections(&provider, serde_json::json!({"rows":[]})).is_err());
        assert!(decode_sections(&provider, serde_json::json!({"sections":[{"id":"x","title":"X","rows":[{"label":"bad","value":3}]}]})).is_err());
        assert!(decode_sections(&provider, serde_json::json!({"sections":[{"id":"x","title":"X","rows":[{"label":"bad","value":"x".repeat(16385)}]}]})).is_err());
    }

    #[test]
    fn built_in_metadata_is_a_stable_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        std::fs::write(&path, b"abc").unwrap();
        let details = file_info_at(path.to_str().unwrap(), path.to_str().unwrap());
        let metadata = details
            .iter()
            .find(|section| section.id == "core.metadata")
            .expect("core.metadata section");
        assert!(metadata.rows.iter().any(|row| row.label == "大小"));
        assert!(metadata.rows.iter().any(|row| row.label == "扩展名"));
        assert_eq!(timestamp_text(0), "1970-01-01 00:00");
    }

    #[test]
    fn native_audio_probe_reads_a_wav_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.wav");
        let data_len = 16_u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_000_u32.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        std::fs::write(&path, bytes).unwrap();
        let sections = media_sections(&path);
        assert!(sections.iter().any(|section| section.title == "音频"));
    }

    #[test]
    fn duration_format_is_stable() {
        assert_eq!(duration_text(Duration::from_secs(65)), "1:05");
        assert_eq!(duration_text(Duration::from_secs(3665)), "1:01:05");
        assert!(!duration_text(Duration::ZERO).is_empty());
    }
}
