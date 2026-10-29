//! Windows Office-to-PDF bridge used by the built-in preview fallback.
//!
//! The plugin still owns document parsing when conversion is unavailable. The
//! converter is intentionally local and lazy: it writes only a Core cache file
//! and never exposes the source path to a plugin or external process.
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::process::Command;

fn cache_path(source: &Path) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let metadata = std::fs::metadata(source).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    digest.update(source.to_string_lossy().as_bytes());
    digest.update([0]);
    digest.update(metadata.len().to_le_bytes());
    digest.update(
        metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_nanos())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    let directory = dirs::cache_dir()
        .ok_or("cache directory unavailable")?
        .join("fiex")
        .join("office-pdf");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory.join(format!("{:x}.pdf", digest.finalize())))
}

#[cfg(windows)]
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(windows)]
fn run_script(script: &str) -> Result<(), String> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("Office converter unavailable: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = format!("{stderr}\n{stdout}");
    Err(if detail.trim().is_empty() {
        "Office conversion failed".into()
    } else {
        detail.chars().take(500).collect()
    })
}

pub fn convert(source: &Path, extension: &str) -> Result<PathBuf, String> {
    let output = cache_path(source)?;
    if output.is_file() && output.metadata().is_ok_and(|metadata| metadata.len() > 0) {
        return Ok(output);
    }
    #[cfg(not(windows))]
    {
        let _ = extension;
        return Err(
            "Office PDF conversion is only available with a desktop Office installation".into(),
        );
    }
    #[cfg(windows)]
    {
        let source = quote(&source.to_string_lossy());
        let target = quote(&output.to_string_lossy());
        let office = if extension.eq_ignore_ascii_case("pptx") {
            "PowerPoint.Application"
        } else {
            "Word.Application"
        };
        let script = if extension.eq_ignore_ascii_case("pptx") {
            format!(
                "$ErrorActionPreference='Stop'; \
                 $app=New-Object -ComObject {office}; \
                 try {{ $doc=$app.Presentations.Open({source},$true,$false,$false); \
                 $doc.SaveAs({target},32); $doc.Close() }} finally {{ $app.Quit() }}"
            )
        } else {
            format!(
                "$ErrorActionPreference='Stop'; \
                 $app=New-Object -ComObject {office}; \
                 $app.Visible=$false; \
                 try {{ $doc=$app.Documents.Open({source},$false,$true); \
                 $doc.ExportAsFixedFormat({target},17); $doc.Close(0) }} \
                 finally {{ $app.Quit() }}"
            )
        };
        run_script(&script)?;
        if output.is_file() && output.metadata().is_ok_and(|metadata| metadata.len() > 0) {
            Ok(output)
        } else {
            Err("Office conversion produced no PDF".into())
        }
    }
}
