use super::{MIB, download_parallelism, header, number, prepare, validate};
use serde_json::{Value, json};

#[test]
fn headers_are_formatted_and_non_object_values_are_ignored() {
    assert_eq!(
        header(&json!({"Accept": "application/octet-stream"})).unwrap(),
        ["Accept: application/octet-stream"]
    );
    assert!(header(&Value::Null).unwrap().is_empty());
    assert!(header(&json!("not a map")).unwrap().is_empty());
}

#[test]
fn headers_reject_injection_oversize_values_and_too_many_entries() {
    for invalid in [
        json!({"X-Test": 1}),
        json!({"": "empty name"}),
        json!({"X-Test\nInjected": "value"}),
        json!({"X-Test": "value\r\nInjected: yes"}),
        json!({"X-Test": "v".repeat(4097)}),
        json!({"x".repeat(257): "value"}),
    ] {
        assert!(
            header(&invalid).is_err(),
            "accepted invalid headers: {invalid}"
        );
    }

    let too_many = (0..17)
        .map(|index| (format!("X-{index}"), Value::String("value".into())))
        .collect();
    assert!(header(&Value::Object(too_many)).is_err());
}

#[test]
fn numeric_options_use_defaults_and_clamp_to_supported_ranges() {
    let params = json!({"negative": -1, "fraction": 1.5, "large": 100});
    assert_eq!(number(&params, "missing", 5, 1, 10), 5);
    assert_eq!(number(&params, "negative", 5, 1, 10), 5);
    assert_eq!(number(&params, "fraction", 5, 1, 10), 5);
    assert_eq!(number(&params, "large", 5, 1, 10), 10);
}

#[test]
fn download_parallelism_respects_size_boundaries() {
    assert_eq!(download_parallelism(None, 16, 16, MIB), (1, 1));
    assert_eq!(download_parallelism(Some(0), 16, 16, MIB), (1, 1));
    assert_eq!(download_parallelism(Some(4 * MIB - 1), 16, 16, MIB), (1, 1));
    assert_eq!(download_parallelism(Some(4 * MIB), 16, 16, MIB), (2, 2));
    assert_eq!(download_parallelism(Some(16 * MIB), 16, 16, MIB), (4, 4));
    assert_eq!(download_parallelism(Some(64 * MIB), 16, 16, MIB), (8, 8));
    assert_eq!(download_parallelism(Some(256 * MIB), 16, 16, MIB), (16, 16));
}

#[test]
fn download_parallelism_respects_caps_and_minimum_range_size() {
    assert_eq!(download_parallelism(Some(256 * MIB), 4, 16, MIB), (4, 4));
    assert_eq!(download_parallelism(Some(256 * MIB), 16, 2, MIB), (2, 2));
    assert_eq!(
        download_parallelism(Some(16 * MIB), 16, 16, 8 * MIB),
        (2, 2)
    );
    assert_eq!(download_parallelism(Some(4 * MIB), 16, 16, 4 * MIB), (1, 1));
}

#[test]
fn preparation_rejects_invalid_urls_and_applies_caps() {
    let directory = tempfile::tempdir().unwrap();
    for invalid in [
        json!({}),
        json!({"url": "file:///tmp/data"}),
        json!({"url": "https://bad\n.test"}),
    ] {
        assert!(validate(&invalid).is_err());
    }

    let request = validate(&json!({
        "url": "https://example.test/archive.bin",
        "size": 64 * MIB,
        "connections": 16,
        "split": 4,
        "minSplitSize": MIB,
        "timeoutMs": 0,
        "allowInvalidCerts": true,
        "headers": {"Accept": "application/octet-stream"}
    }))
    .unwrap();
    let prepared = prepare(request, directory.path());

    assert_eq!(prepared.url, "https://example.test/archive.bin");
    assert_eq!(prepared.size, Some(64 * MIB));
    assert!(prepared.segmented);
    assert_eq!(prepared.options.split, Some(4));
    assert_eq!(prepared.options.max_connection_per_server, Some(4));
    assert_eq!(prepared.options.timeout, Some(1));
    assert!(!prepared.options.check_certificate);
    assert_eq!(
        prepared.options.header,
        ["Accept: application/octet-stream"]
    );
}
