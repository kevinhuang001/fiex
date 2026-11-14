use super::{MediaSection, duration_text, row};
use crate::Localization;
use std::path::Path;

pub(super) fn matroska_section(path: &Path, localization: &Localization) -> Option<MediaSection> {
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
