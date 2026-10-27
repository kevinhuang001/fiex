use super::{row, InfoSection};
use crate::plugins::contracts::ResourceMetadata;
use std::path::Path;
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

pub(super) fn timestamp_text(seconds: i64) -> String {
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

/// Sum file lengths below a local directory for the Info sheet. Symlinks are
/// skipped so a link cycle cannot trap the walk; unreadable entries count 0.
pub(super) fn directory_size(path: &Path) -> Option<u64> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_dir() {
        return None;
    }
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total = total.saturating_add(metadata.len());
                }
            }
        }
    }
    Some(total)
}

pub(super) fn metadata_section(
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
            crate::i18n::t("种类"),
            projected
                .map(|value| match value.kind.as_str() {
                    "directory" => crate::i18n::t("文件夹"),
                    _ => crate::i18n::t("文件"),
                })
                .unwrap_or(if is_dir {
                    crate::i18n::t("文件夹")
                } else {
                    crate::i18n::t("文件")
                }),
        ),
        row(
            crate::i18n::t("大小"),
            if is_dir {
                directory_size(local)
                    .map(size_text)
                    .unwrap_or_else(|| "—".into())
            } else {
                projected
                    .and_then(|value| value.size)
                    .or_else(|| metadata.as_ref().map(|value| value.len()))
                    .map(size_text)
                    .unwrap_or_else(|| "—".into())
            },
        ),
        row(crate::i18n::t("位置"), display_parent(display)),
    ];
    if !extension.is_empty() {
        rows.push(row(crate::i18n::t("扩展名"), extension));
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
        rows.push(row(crate::i18n::t("修改时间"), timestamp_text(modified)));
    }
    if let Some(permissions) = metadata.as_ref().map(|value| value.permissions()) {
        rows.push(row(
            crate::i18n::t("权限"),
            if permissions.readonly() {
                crate::i18n::t("只读")
            } else {
                crate::i18n::t("可读写")
            },
        ));
    }
    if let Some(link) = metadata
        .as_ref()
        .filter(|value| value.file_type().is_symlink())
        .and_then(|_| std::fs::read_link(local).ok())
    {
        rows.push(row(crate::i18n::t("符号链接"), link.to_string_lossy()));
    }
    InfoSection {
        id: "core.metadata".into(),
        source: "core".into(),
        title: crate::i18n::t("文件").into(),
        plugin_id: None,
        provider: "Fiex".into(),
        rows,
    }
}
