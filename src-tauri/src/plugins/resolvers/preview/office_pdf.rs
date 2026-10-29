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
#[cfg(windows)]
fn run_script(script: &str) -> Result<(), String> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Office converter unavailable: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Office conversion timed out".into());
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut detail = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut detail);
    }
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
        Err("Office PDF conversion is only available with a desktop Office installation".into())
    }
    #[cfg(windows)]
    {
        let source = quote(&source.to_string_lossy());
        let target = quote(&output.to_string_lossy());
        let script = if extension.eq_ignore_ascii_case("pptx") {
            format!(
                "$ErrorActionPreference='Stop'; \
                 try {{ $app=New-Object -ComObject PowerPoint.Application }} \
                 catch {{ $app=New-Object -ComObject KWPP.Application }}; \
                 $app.DisplayAlerts=0; \
                 try {{ $doc=$app.Presentations.Open({source},$true,$false,$false); \
                 $doc.SaveAs({target},32); try {{ $doc.Close() }} catch {{}} }} \
                 finally {{ try {{ $app.Quit() }} catch {{}} }}; exit 0"
            )
        } else {
            format!(
                "$ErrorActionPreference='Stop'; \
                 try {{ $app=New-Object -ComObject KWPS.Application }} \
                 catch {{ $app=New-Object -ComObject Word.Application }}; \
                 $app.Visible=$false; $app.DisplayAlerts=0; \
                 try {{ $doc=$app.Documents.Open({source},$false,$true); \
                 $doc.ExportAsFixedFormat({target},17); try {{ $doc.Close(0) }} catch {{}} }} \
                 finally {{ try {{ $app.Quit() }} catch {{}} }}; exit 0"
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
