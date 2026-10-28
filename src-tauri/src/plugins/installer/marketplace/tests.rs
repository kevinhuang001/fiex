use super::*;
#[test]
fn reject_untrusted_repository_names() {
    assert!(parse_repo("owner/repo").is_ok());
    for name in [
        "owner/repo/extra",
        "../repo",
        "owner/../repo",
        "owner/repo?x=1",
        "",
        "https://github.com/x/y",
    ] {
        assert!(parse_repo(name).is_err(), "{name}");
    }
}
#[test]
fn release_listing_can_offer_multiple_plugins_per_platform() {
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let release = serde_json::json!({"assets":[
        {"name":format!("archive-{platform}.fiexplugin")},
        {"name":format!("git-{platform}.fiexplugin")},
        {"name":"other-platform-only.fiexplugin"}
    ]});
    assert_eq!(compatible_assets(&release).len(), 2);
}

#[cfg(test)]
mod description_tests {
    use super::*;

    #[test]
    fn parses_release_descriptions_by_plugin_id() {
        let map = parse_descriptions(
            r#"{"schemaVersion":2,"plugins":[{"id":"org.fiex.git","description":"源代码管理"},{"id":"bad id","description":"x"}]}"#.as_bytes(),
        );
        assert_eq!(
            map.get("org.fiex.git").map(String::as_str),
            Some("源代码管理")
        );
        assert!(!map.contains_key("bad id"));
    }

    #[test]
    fn rejects_malformed_descriptions() {
        assert!(parse_descriptions(b"not json").is_empty());
    }
}
