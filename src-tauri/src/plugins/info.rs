//! "Show info" metadata: Core-native media probes plus plugin extensions.
//!
//! Plugins declare `extensions.info` entries with a command, optional
//! extensions and a title. Core invokes a matching plugin for the selected file
//! through the normal operation-scoped plugin bridge, then renders the returned
//! rows beside the native file facts.
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoRow {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoSection {
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
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
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

fn extension_matches(contribution: &Value, ext: &str, is_dir: bool) -> bool {
    let Some(values) = contribution.get("extensions").and_then(Value::as_array) else {
        return !is_dir;
    };
    if values.is_empty() {
        return !is_dir;
    }
    values.iter().filter_map(Value::as_str).any(|value| {
        let value = value
            .trim_start_matches("*.")
            .trim_start_matches('.')
            .to_ascii_lowercase();
        value == "*" || value == ext
    })
}

fn contribution_rows(value: &Value) -> Vec<InfoRow> {
    let Some(rows) = value.get("rows").and_then(Value::as_array) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|entry| {
            if let Some(object) = entry.as_object() {
                let label = object
                    .get("label")
                    .or_else(|| object.get("key"))
                    .map(value_text)
                    .unwrap_or_default();
                let value = object.get("value").map(value_text).unwrap_or_default();
                (!label.is_empty() && !value.is_empty()).then_some(row(label, value))
            } else if let Some(pair) = entry.as_array() {
                (pair.len() >= 2).then(|| row(value_text(&pair[0]), value_text(&pair[1])))
            } else {
                None
            }
        })
        .collect()
}

fn plugin_sections(path: &Path) -> Vec<InfoSection> {
    let ext = extension(path);
    let is_dir = path.is_dir();
    let directory = path.parent().unwrap_or(Path::new("."));
    let mut sections = Vec::new();
    for plugin in crate::plugin_host::catalog() {
        if !plugin.enabled || !plugin.approved || !plugin.backend_available {
            continue;
        }
        let declared = plugin
            .declared_permissions
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if !crate::plugin_permissions::allowed(&plugin.id, &declared, "metadata").unwrap_or(false)
            || !crate::plugin_permissions::allowed(&plugin.id, &declared, "files.read").unwrap_or(false)
        {
            continue;
        }
        let Some(extensions) = plugin.extensions.get("info").and_then(Value::as_array) else {
            continue;
        };
        let mut matching = extensions
            .iter()
            .filter(|contribution| extension_matches(contribution, &ext, is_dir))
            .collect::<Vec<_>>();
        matching.sort_by_key(|contribution| {
            std::cmp::Reverse(
                contribution
                    .get("priority")
                    .and_then(Value::as_u64)
                    .unwrap_or(50),
            )
        });
        for contribution in matching {
            let Some(command) = contribution.get("command").and_then(Value::as_str) else {
                continue;
            };
            let title = contribution
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(&plugin.name)
                .to_string();
            let selected = [PathBuf::from(path)];
            let resource_name = path.file_name().unwrap_or_default().to_string_lossy();
            let params = json!({
                "path": resource_name,
                "name": resource_name,
                "isDir": is_dir,
                "resource": {
                    "name": resource_name,
                    "extension": ext,
                    "kind": if is_dir { "directory" } else { "file" },
                },
            });
            let result = crate::plugin_host::global().invoke_selected(
                &plugin.id,
                command,
                params,
                &directory.to_string_lossy(),
                &selected,
            );
            match result {
                Ok(value) => {
                    let rows = contribution_rows(&value);
                    if !rows.is_empty() {
                        sections.push(InfoSection {
                            source: "plugin".into(),
                            title,
                            plugin_id: Some(plugin.id.clone()),
                            provider: plugin.name.clone(),
                            rows,
                        });
                    }
                }
                Err(error) => sections.push(InfoSection {
                    source: "plugin".into(),
                    title,
                    plugin_id: Some(plugin.id.clone()),
                    provider: plugin.name.clone(),
                    rows: vec![row("错误", error)],
                }),
            }
        }
    }
    sections
}

pub fn file_info(path: &str) -> Vec<InfoSection> {
    // Remote/VFS paths are served by their own plugin and do not map to a
    // local file that Core or another plugin may probe.
    if path.contains("://") {
        return Vec::new();
    }
    let path = Path::new(path);
    let mut sections = media_sections(path);
    sections.extend(plugin_sections(path));
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_matching_handles_wildcards_and_directories() {
        assert!(extension_matches(&json!({}), "txt", false));
        assert!(!extension_matches(&json!({}), "txt", true));
        assert!(extension_matches(
            &json!({"extensions": ["*.ZIP", "rar"]}),
            "zip",
            false
        ));
        assert!(!extension_matches(
            &json!({"extensions": ["zip"]}),
            "rar",
            false
        ));
    }

    #[test]
    fn contribution_rows_accept_objects_and_pairs() {
        let rows = contribution_rows(&json!({
            "rows": [
                {"label": "SHA-256", "value": "abc"},
                ["大小", "3"]
            ]
        }));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "SHA-256");
        assert_eq!(rows[1].value, "3");
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
