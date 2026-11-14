use super::{MediaSection, duration_text, row};
use crate::Localization;
use std::{fs, path::Path};

pub(super) fn mp4_section(path: &Path, localization: &Localization) -> Option<MediaSection> {
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
