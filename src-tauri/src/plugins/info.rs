//! "Show info" registry: built-in providers plus standard `info.resolve`.
//!
//! Built-in and plugin providers return the same section/row domain types.
//! Core builds an `EventContext` after permission projection, invokes the
//! matching provider, and merges the decoded `InfoResponse` with native
//! metadata and media probes.
use crate::plugin_contracts::{EventContext, InfoResponse, ResourceMetadata};
use crate::plugin_resource_context::ResourceSnapshot;
use serde::Serialize;
use std::{
    fs,
    path::Path,
    time::Duration,
};

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

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn display_name(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(path)
        .to_string()
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

fn metadata_section(
    display: &str,
    local: &Path,
    metadata: Option<&std::fs::Metadata>,
    projected: Option<&ResourceMetadata>,
) -> InfoSection {
    let is_dir = metadata.as_ref().is_some_and(|value| value.is_dir());
    let name = projected
        .map(|value| value.name.clone())
        .unwrap_or_else(|| display_name(display));
    let extension = projected
        .and_then(|value| value.extension.clone())
        .unwrap_or_else(|| {
            Path::new(&name)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
        });
    let mut rows = vec![
        row(
            "种类",
            projected
                .map(|value| match value.kind.as_str() {
                    "directory" => "文件夹",
                    _ => "文件",
                })
                .unwrap_or(if is_dir { "文件夹" } else { "文件" }),
        ),
        row(
            "大小",
            if is_dir {
                "—".to_string()
            } else {
                projected
                    .and_then(|value| value.size)
                    .or_else(|| metadata.as_ref().map(|value| value.len()))
                    .map(size_text)
                    .unwrap_or_else(|| "—".into())
            },
        ),
        row("位置", display_parent(display)),
    ];
    if !extension.is_empty() {
        rows.push(row("扩展名", extension));
    }
    if let Some(mime) = projected.and_then(|value| value.mime.clone()) {
        rows.push(row("MIME", mime));
    }
    if let Some(modified) = projected.and_then(|value| value.modified).or_else(|| {
        metadata
            .as_ref()
            .and_then(|value| value.modified().ok())
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_secs() as i64)
    }) {
        rows.push(row("修改时间", timestamp_text(modified)));
    }
    if let Some(permissions) = metadata.as_ref().map(|value| value.permissions()) {
        rows.push(row(
            "权限",
            if permissions.readonly() {
                "只读"
            } else {
                "可读写"
            },
        ));
    }
    if let Some(link) = metadata
        .as_ref()
        .filter(|value| value.file_type().is_symlink())
        .and_then(|_| std::fs::read_link(local).ok())
    {
        rows.push(row("符号链接", link.to_string_lossy()));
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

fn decode_response(value: &serde_json::Value) -> Option<InfoResponse> {
    match serde_json::from_value::<InfoResponse>(value.clone()) {
        Ok(response) => Some(response),
        Err(error) => {
            eprintln!("invalid info response: {error}");
            None
        }
    }
}

enum Candidate {
    Metadata,
    Media,
    Plugin {
        plugin: Box<crate::plugin_host::PluginInfo>,
        contribution: serde_json::Value,
        priority: i64,
        metadata_allowed: bool,
        read_allowed: bool,
    },
}

impl Candidate {
    fn priority(&self) -> i64 {
        match self {
            Self::Metadata => 1000,
            Self::Media => 100,
            Self::Plugin { priority, .. } => *priority,
        }
    }
}

fn candidates(resource: &ResourceSnapshot) -> Vec<Candidate> {
    let mut values = vec![Candidate::Metadata];
    if !resource.is_dir {
        values.push(Candidate::Media);
    }
    for registered in crate::plugin_events::matching_plugins(
        "info.resolve",
        &resource.ext,
        &resource.mime,
        resource.is_dir,
    ) {
        let plugin = registered.plugin;
        let declared = plugin
            .declared_permissions
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let metadata_allowed =
            crate::plugin_permissions::allowed(&plugin.id, &declared, "metadata").unwrap_or(false);
        let read_allowed = crate::plugin_permissions::allowed(&plugin.id, &declared, "files.read")
            .unwrap_or(false);
        let contribution = registered.contribution;
        values.push(Candidate::Plugin {
            plugin: Box::new(plugin.clone()),
            contribution,
            priority: registered.priority,
            metadata_allowed,
            read_allowed,
        });
    }
    values.sort_by_key(|candidate| std::cmp::Reverse(candidate.priority()));
    values
}

fn section_from_contract(
    plugin: Option<&crate::plugin_host::PluginInfo>,
    section: crate::plugin_contracts::InfoSectionView,
) -> Option<InfoSection> {
    let rows = section
        .rows
        .into_iter()
        .map(|row| InfoRow {
            label: row.label,
            value: row.value,
            copyable: row.copyable,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    Some(InfoSection {
        id: match plugin {
            Some(plugin) => format!("{}.{}", plugin.id, section.id),
            None => section.id,
        },
        source: if plugin.is_some() { "plugin" } else { "core" }.into(),
        title: section.title,
        plugin_id: plugin.map(|plugin| plugin.id.clone()),
        provider: plugin
            .map(|plugin| plugin.name.clone())
            .unwrap_or_else(|| "Fiex".into()),
        rows,
    })
}

fn resolve_builtin(
    candidate: &Candidate,
    context: &EventContext,
    local: &Path,
    display: &str,
    resource_stat: Option<&std::fs::Metadata>,
) -> Vec<InfoSection> {
    match candidate {
        Candidate::Metadata => vec![metadata_section(
            display,
            local,
            resource_stat,
            context
                .targets
                .first()
                .and_then(|target| target.metadata.as_ref()),
        )],
        Candidate::Media => media_sections(local),
        Candidate::Plugin { .. } => Vec::new(),
    }
}

fn resolve_plugin(
    plugin: &crate::plugin_host::PluginInfo,
    _contribution: &serde_json::Value,
    context: &EventContext,
    local: &Path,
) -> Vec<InfoSection> {
    let params = serde_json::to_value(context).unwrap_or(serde_json::Value::Null);
    let directory = local.parent().unwrap_or(Path::new("."));
    let selected = [local.to_path_buf()];
    match crate::plugin_events::invoke_selected(
        &plugin.id,
        "info.resolve",
        params,
        &directory.to_string_lossy(),
        &selected,
    ) {
        Ok(value) => {
            if let Some(response) = decode_response(&value) {
                response
                    .sections
                    .into_iter()
                    .filter_map(|section| section_from_contract(Some(plugin), section))
                    .collect()
            } else {
                vec![InfoSection {
                    id: format!("{}.invalid", plugin.id),
                    source: "plugin".into(),
                    title: plugin.name.clone(),
                    plugin_id: Some(plugin.id.clone()),
                    provider: plugin.name.clone(),
                    rows: vec![row("错误", "插件返回的 InfoResponse 格式无效")],
                }]
            }
        }
        Err(error) => vec![InfoSection {
            id: format!("{}.error", plugin.id),
            source: "plugin".into(),
            title: plugin.name.clone(),
            plugin_id: Some(plugin.id.clone()),
            provider: plugin.name.clone(),
            rows: vec![row("错误", error)],
        }],
    }
}

/// Resolve info for a local path. `display` preserves the user-facing path for
/// remote materializations; `local` is the Core-owned file used for probes.
pub fn file_info_at(display: &str, local: &str) -> Vec<InfoSection> {
    let local_path = Path::new(local);
    let resource = ResourceSnapshot::new(display, local_path);
    let mut sections = Vec::new();
    for candidate in candidates(&resource) {
        match &candidate {
            Candidate::Metadata | Candidate::Media => {
                let context = resource.context("__core__", true, !resource.is_dir);
                let mut resolved = resolve_builtin(&candidate, &context, local_path, display, resource.stat.as_ref());
                if let Candidate::Metadata = candidate {
                    // Use the display path Core originally received.
                    if let Some(section) = resolved.first_mut() {
                        if let Some(location) =
                            section.rows.iter_mut().find(|row| row.label == "位置")
                        {
                            location.value = display_parent(display);
                        }
                    }
                }
                sections.append(&mut resolved);
            }
            Candidate::Plugin {
                plugin,
                contribution,
                metadata_allowed,
                read_allowed,
                ..
            } => {
                let context = resource.context(&plugin.id, *metadata_allowed, *read_allowed);
                sections.extend(resolve_plugin(plugin, contribution, &context, local_path));
            }
        }
    }
    sections
}

pub fn file_info(path: &str) -> Vec<InfoSection> {
    file_info_at(path, path)
}

#[cfg(test)]
mod tests {
    use super::*;

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
