//! Validated setup instructions and generated private credential declarations.
use serde_json::{Map, Value};
pub(super) fn validate(object: &Map<String, Value>) -> Result<(), String> {
    if object
        .get("description")
        .is_some_and(|v| v.as_str().is_none_or(|s| s.len() > 4096))
    {
        return Err("invalid connection setup description".into());
    }
    if let Some(url) = object.get("setupUrl") {
        let parsed = reqwest::Url::parse(url.as_str().ok_or("invalid connection setup URL")?)
            .map_err(|_| "invalid connection setup URL")?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("connection setup URL must use HTTPS".into());
        }
    }
    if let Some(keys) = object.get("credentials") {
        let keys = keys
            .as_array()
            .filter(|keys| keys.len() <= 32)
            .ok_or("invalid private connection credentials")?;
        let mut seen = std::collections::BTreeSet::new();
        for key in keys {
            let key = key
                .as_str()
                .filter(|key| {
                    !key.is_empty()
                        && key.len() <= 80
                        && key
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                })
                .ok_or("invalid private credential name")?;
            if !seen.insert(key) {
                return Err("duplicate private credential name".into());
            }
        }
    }

    Ok(())
}
