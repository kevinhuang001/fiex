use chrono::{TimeZone, Utc};
use fiex_provider_contracts::ResourceMetadata;
use std::{fs, path::Path};

pub fn timestamp_text(seconds: i64) -> String {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".into())
}

pub(super) fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B ({bytes} bytes)")
    } else {
        format!("{value:.1} {} ({bytes} bytes)", UNITS[unit])
    }
}

pub(super) fn display_parent(path: &str) -> String {
    path.trim_end_matches(['/', '\\'])
        .rsplit_once(['/', '\\'])
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

pub(super) fn resource_kind<'a>(
    stat: Option<&fs::Metadata>,
    projected: Option<&'a ResourceMetadata>,
) -> &'a str {
    projected
        .map(|value| value.kind.as_str())
        .or_else(|| {
            stat.map(|value| {
                if value.file_type().is_symlink() {
                    "symlink"
                } else if value.is_dir() {
                    "directory"
                } else {
                    "file"
                }
            })
        })
        .unwrap_or("file")
}

pub(super) fn fallback_name(display: &str) -> String {
    Path::new(display)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| display.to_string())
}
