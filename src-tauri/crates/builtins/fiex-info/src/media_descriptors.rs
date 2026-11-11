use crate::Localization;
use fiex_provider_contracts::{
    InfoCompute, InfoFieldDescriptor, InfoProviderPresentation, InfoSubsectionDescriptor,
    InfoSubsectionMatch, ResourceMetadata,
};
use std::path::Path;

fn field(id: &str, label: String) -> InfoFieldDescriptor {
    InfoFieldDescriptor {
        id: id.into(),
        label,
        compute: InfoCompute::OnDemand,
    }
}

fn descriptor(
    id: &str,
    fields: impl IntoIterator<Item = InfoFieldDescriptor>,
) -> InfoSubsectionDescriptor {
    InfoSubsectionDescriptor {
        id: id.into(),
        r#match: None,
        fields: fields.into_iter().collect(),
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .trim_start_matches('.')
        .to_ascii_lowercase()
}

pub fn media_descriptors(
    path: &Path,
    metadata: Option<&ResourceMetadata>,
    localization: &Localization,
) -> InfoProviderPresentation {
    const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov", "mkv", "webm"];
    const AUDIO_EXTENSIONS: &[&str] = &[
        "mp3", "flac", "m4a", "aac", "ogg", "oga", "opus", "wav", "wave", "aiff", "aif", "ape",
        "wv", "wma",
    ];
    let ext = metadata
        .and_then(|metadata| metadata.extension.as_deref())
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
        .unwrap_or_else(|| extension(path));
    let mime = metadata
        .and_then(|metadata| metadata.mime.as_deref())
        .unwrap_or_default();
    let video = VIDEO_EXTENSIONS.contains(&ext.as_str())
        || (ext.is_empty() && mime.to_ascii_lowercase().starts_with("video/"));
    let audio = AUDIO_EXTENSIONS.contains(&ext.as_str())
        || (ext.is_empty() && mime.to_ascii_lowercase().starts_with("audio/"));
    let (id, extensions, mime_types, fields) = if video {
        (
            "video",
            if ext.is_empty() {
                Vec::new()
            } else {
                VIDEO_EXTENSIONS
                    .iter()
                    .map(|value| (*value).into())
                    .collect()
            },
            if ext.is_empty() {
                vec!["video/*".into()]
            } else {
                Vec::new()
            },
            vec![
                field("duration", localization.text("时长")),
                field("video", localization.text("视频")),
                field("audio", localization.text("音频")),
                field("container", localization.text("容器")),
            ],
        )
    } else if audio {
        (
            "audio",
            if ext.is_empty() {
                Vec::new()
            } else {
                AUDIO_EXTENSIONS
                    .iter()
                    .map(|value| (*value).into())
                    .collect()
            },
            if ext.is_empty() {
                vec!["audio/*".into()]
            } else {
                Vec::new()
            },
            vec![
                field("format", localization.text("格式")),
                field("title", localization.text("标题")),
                field("artist", localization.text("艺术家")),
                field("album", localization.text("专辑")),
                field("genre", localization.text("流派")),
                field("year", localization.text("年份")),
                field("track", localization.text("音轨")),
                field("disk", localization.text("碟片")),
                field("duration", localization.text("时长")),
                field("bitrate", localization.text("码率")),
                field("sampleRate", localization.text("采样率")),
                field("channels", localization.text("声道")),
            ],
        )
    } else {
        return InfoProviderPresentation {
            title: localization.text("媒体"),
            subsections: Vec::new(),
        };
    };
    let mut subsection = descriptor(id, fields);
    subsection.r#match = Some(InfoSubsectionMatch {
        extensions,
        mime_types,
        ..Default::default()
    });
    InfoProviderPresentation {
        title: localization.text("媒体"),
        subsections: vec![subsection],
    }
}
