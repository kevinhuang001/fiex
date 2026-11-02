use super::media::duration_text;
use super::metadata::{directory_size, timestamp_text};
use super::*;
use std::time::Duration;

#[test]
fn built_in_metadata_is_a_stable_section() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hello.txt");
    std::fs::write(&path, b"abc").unwrap();
    let details = file_info_resource(path.to_str().unwrap()).unwrap();
    let metadata = details
        .iter()
        .find(|section| section.id == "core.metadata")
        .expect("core.metadata section");
    assert!(metadata.rows.iter().any(|row| row.label == "大小"));
    assert!(metadata.rows.iter().any(|row| row.label == "扩展名"));
    let sub = dir.path().join("nested");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("part.bin"), b"12345").unwrap();
    let size = directory_size(dir.path()).unwrap();
    assert!(size >= 5, "{size}");
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
